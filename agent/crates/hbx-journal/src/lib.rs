use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use crc32fast::Hasher as Crc32Hasher;
use hbx_core::domain::common::{Checkpoint, JobId};
use hbx_core::pipeline::{IJournal, JournalEntry, JournalError};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

const RECORD_HEADER_SIZE: usize = 1 + 8 + 16 + 4;
const RECORD_FOOTER_SIZE: usize = 4;
const DEFAULT_ROTATION_SIZE: u64 = 16 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct CheckpointPayload {
    pub progress: f64,
    pub pending_files: usize,
}

pub struct AppendJournal {
    file: Mutex<File>,
    path: PathBuf,
    rotation_size: u64,
    current_size: Mutex<u64>,
}

impl AppendJournal {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, JournalError> {
        Self::open_with_rotation(path, DEFAULT_ROTATION_SIZE)
    }

    pub fn open_with_rotation(
        path: impl Into<PathBuf>,
        rotation_size: u64,
    ) -> Result<Self, JournalError> {
        let path = path.into();
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&path)?;
        let current_size = file.metadata()?.len();

        Ok(Self {
            file: Mutex::new(file),
            path,
            rotation_size,
            current_size: Mutex::new(current_size),
        })
    }

    fn encode_entry(entry: &JournalEntry) -> Result<Vec<u8>, JournalError> {
        let (entry_type, job_id, timestamp, payload) = match entry {
            JournalEntry::TaskStarted {
                job_id,
                execution_id,
                timestamp,
            } => (1u8, job_id, *timestamp, serde_json::to_vec(execution_id)?),
            JournalEntry::FileProcessed {
                job_id,
                file_path,
                chunks,
            } => {
                let payload = serde_json::to_vec(&(file_path, chunks))?;
                (2u8, job_id, 0, payload)
            }
            JournalEntry::ChunkWritten {
                job_id,
                hash,
                location,
            } => {
                let payload = serde_json::to_vec(&(hash, location))?;
                (3u8, job_id, 0, payload)
            }
            JournalEntry::Checkpoint {
                job_id,
                progress,
                pending_files,
            } => {
                let payload = serde_json::to_vec(&CheckpointPayload {
                    progress: *progress,
                    pending_files: *pending_files,
                })?;
                (4u8, job_id, 0, payload)
            }
            JournalEntry::TaskCompleted {
                job_id,
                version_id,
                result,
            } => {
                let payload = serde_json::to_vec(&(version_id, result))?;
                (5u8, job_id, 0, payload)
            }
            JournalEntry::TaskFailed { job_id, error } => {
                let payload = serde_json::to_vec(error)?;
                (6u8, job_id, 0, payload)
            }
        };

        let job_id_bytes = *job_id.0.as_bytes();

        let mut record = Vec::with_capacity(RECORD_HEADER_SIZE + payload.len() + RECORD_FOOTER_SIZE);
        record.push(entry_type);
        record.extend_from_slice(&timestamp.to_le_bytes());
        record.extend_from_slice(&job_id_bytes);
        record.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        record.extend_from_slice(&payload);

        let mut crc = Crc32Hasher::new();
        crc.update(&record);
        let crc_val = crc.finalize();
        record.extend_from_slice(&crc_val.to_le_bytes());

        Ok(record)
    }

    fn decode_entry(data: &[u8]) -> Result<JournalEntry, JournalError> {
        if data.len() < RECORD_HEADER_SIZE + RECORD_FOOTER_SIZE {
            return Err(JournalError::Corrupted);
        }

        let entry_type = data[0];
        let timestamp = u64::from_le_bytes(data[1..9].try_into().unwrap());
        let job_id_bytes: [u8; 16] = data[9..25].try_into().unwrap();
        let payload_len =
            u32::from_le_bytes(data[25..29].try_into().unwrap()) as usize;

        if data.len() < RECORD_HEADER_SIZE + payload_len + RECORD_FOOTER_SIZE {
            return Err(JournalError::Corrupted);
        }

        let payload = &data[RECORD_HEADER_SIZE..RECORD_HEADER_SIZE + payload_len];
        let crc_bytes =
            &data[RECORD_HEADER_SIZE + payload_len..RECORD_HEADER_SIZE + payload_len + 4];
        let stored_crc = u32::from_le_bytes(crc_bytes.try_into().unwrap());

        let mut crc = Crc32Hasher::new();
        crc.update(&data[..RECORD_HEADER_SIZE + payload_len]);
        let computed_crc = crc.finalize();

        if stored_crc != computed_crc {
            return Err(JournalError::Corrupted);
        }

        let job_id = JobId(uuid::Uuid::from_bytes(job_id_bytes));

        match entry_type {
            1 => {
                let execution_id: hbx_core::domain::common::ExecutionId =
                    serde_json::from_slice(payload)?;
                Ok(JournalEntry::TaskStarted {
                    job_id,
                    execution_id,
                    timestamp,
                })
            }
            2 => {
                let (file_path, chunks): (String, Vec<hbx_core::domain::chunk::ChunkHash>) =
                    serde_json::from_slice(payload)?;
                Ok(JournalEntry::FileProcessed {
                    job_id,
                    file_path,
                    chunks,
                })
            }
            3 => {
                let (hash, location): (
                    hbx_core::domain::chunk::ChunkHash,
                    hbx_core::domain::chunk::ChunkLocation,
                ) = serde_json::from_slice(payload)?;
                Ok(JournalEntry::ChunkWritten {
                    job_id,
                    hash,
                    location,
                })
            }
            4 => {
                let cp: CheckpointPayload = serde_json::from_slice(payload)?;
                Ok(JournalEntry::Checkpoint {
                    job_id,
                    progress: cp.progress,
                    pending_files: cp.pending_files,
                })
            }
            5 => {
                let (version_id, result): (
                    hbx_core::domain::common::VersionId,
                    hbx_core::domain::backup::BackupResult,
                ) = serde_json::from_slice(payload)?;
                Ok(JournalEntry::TaskCompleted {
                    job_id,
                    version_id,
                    result,
                })
            }
            6 => {
                let error: hbx_core::domain::backup::BackupError =
                    serde_json::from_slice(payload)?;
                Ok(JournalEntry::TaskFailed { job_id, error })
            }
            _ => Err(JournalError::Corrupted),
        }
    }

    fn read_all_entries(&self) -> Result<Vec<JournalEntry>, JournalError> {
        let mut file = self.file.lock();
        file.seek(SeekFrom::Start(0))?;

        let mut data = Vec::new();
        file.read_to_end(&mut data)?;

        let mut entries = Vec::new();
        let mut offset = 0;

        while offset < data.len() {
            if data.len() - offset < RECORD_HEADER_SIZE + RECORD_FOOTER_SIZE {
                break;
            }

            let payload_len =
                u32::from_le_bytes(data[offset + 25..offset + 29].try_into().unwrap()) as usize;
            let record_len = RECORD_HEADER_SIZE + payload_len + RECORD_FOOTER_SIZE;

            if offset + record_len > data.len() {
                break;
            }

            match Self::decode_entry(&data[offset..offset + record_len]) {
                Ok(entry) => entries.push(entry),
                Err(e) => {
                    tracing::warn!("journal decode error at offset {}: {}", offset, e);
                    break;
                }
            }

            offset += record_len;
        }

        file.seek(SeekFrom::End(0))?;
        Ok(entries)
    }
}

impl IJournal for AppendJournal {
    fn append(&self, entry: JournalEntry) -> Result<(), JournalError> {
        let record = Self::encode_entry(&entry)?;
        let mut file = self.file.lock();
        file.write_all(&record)?;
        file.flush()?;

        let mut size = self.current_size.lock();
        *size += record.len() as u64;

        if *size >= self.rotation_size {
            drop(file);
            drop(size);
            self.rotate()?;
        }

        Ok(())
    }

    fn read_recent(&self, n: usize) -> Result<Vec<JournalEntry>, JournalError> {
        let entries = self.read_all_entries()?;
        let start = entries.len().saturating_sub(n);
        Ok(entries[start..].to_vec())
    }

    fn read_checkpoint(&self, job_id: &JobId) -> Result<Option<Checkpoint>, JournalError> {
        let entries = self.read_all_entries()?;
        let mut last_checkpoint = None;

        for entry in entries.iter().rev() {
            if let JournalEntry::Checkpoint {
                job_id: entry_job_id,
                progress,
                pending_files,
            } = entry
            {
                if entry_job_id == job_id {
                    last_checkpoint = Some(Checkpoint {
                        progress: *progress,
                        pending_files: *pending_files,
                        timestamp: chrono::Utc::now(),
                    });
                    break;
                }
            }
        }

        Ok(last_checkpoint)
    }

    fn rotate(&self) -> Result<(), JournalError> {
        let mut file = self.file.lock();
        let rotated_path = self.path.with_extension("journal.rotated");

        std::fs::rename(&self.path, &rotated_path)?;

        *file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&self.path)?;

        let mut size = self.current_size.lock();
        *size = 0;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbx_core::domain::common::{ExecutionId, JobId};
    use hbx_core::pipeline::IJournal;

    #[test]
    fn test_append_and_read() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let journal = AppendJournal::open(tmp.path()).unwrap();

        let job_id = JobId(uuid::Uuid::new_v4());
        let exec_id = ExecutionId(uuid::Uuid::new_v4());

        journal
            .append(JournalEntry::TaskStarted {
                job_id: job_id.clone(),
                execution_id: exec_id,
                timestamp: 1234567890,
            })
            .unwrap();

        journal
            .append(JournalEntry::Checkpoint {
                job_id: job_id.clone(),
                progress: 0.5,
                pending_files: 100,
            })
            .unwrap();

        let recent = journal.read_recent(10).unwrap();
        assert_eq!(recent.len(), 2);
    }

    #[test]
    fn test_read_checkpoint() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let journal = AppendJournal::open(tmp.path()).unwrap();

        let job_id = JobId(uuid::Uuid::new_v4());

        journal
            .append(JournalEntry::Checkpoint {
                job_id: job_id.clone(),
                progress: 0.3,
                pending_files: 200,
            })
            .unwrap();

        journal
            .append(JournalEntry::Checkpoint {
                job_id: job_id.clone(),
                progress: 0.7,
                pending_files: 50,
            })
            .unwrap();

        let checkpoint = journal.read_checkpoint(&job_id).unwrap();
        assert!(checkpoint.is_some());
        let cp = checkpoint.unwrap();
        assert_eq!(cp.progress, 0.7);
        assert_eq!(cp.pending_files, 50);
    }

    #[test]
    fn test_checkpoint_not_found() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let journal = AppendJournal::open(tmp.path()).unwrap();

        let job_id = JobId(uuid::Uuid::new_v4());
        let checkpoint = journal.read_checkpoint(&job_id).unwrap();
        assert!(checkpoint.is_none());
    }

    #[test]
    fn test_crc_corruption_detection() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let journal = AppendJournal::open(tmp.path()).unwrap();

        let job_id = JobId(uuid::Uuid::new_v4());
        journal
            .append(JournalEntry::Checkpoint {
                job_id: job_id.clone(),
                progress: 0.5,
                pending_files: 10,
            })
            .unwrap();

        let mut data = std::fs::read(tmp.path()).unwrap();
        data[0] ^= 0xFF;
        std::fs::write(tmp.path(), &data).unwrap();

        let journal2 = AppendJournal::open(tmp.path()).unwrap();
        let recent = journal2.read_recent(10).unwrap();
        assert!(recent.is_empty());
    }
}
