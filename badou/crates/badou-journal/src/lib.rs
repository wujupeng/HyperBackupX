//! append-only Journal：崩溃恢复与断点续作。

pub mod entry;
pub mod recovery;

pub use entry::{BadouJournalEntry, JournalOpType};
pub use recovery::{scan_uncompleted, UncompletedOp, RecoveryAction};

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use chrono::Utc;
use crc32fast::Hasher as Crc32Hasher;
use parking_lot::Mutex;
use thiserror::Error;
use uuid::Uuid;


const RECORD_HEADER_SIZE: usize = 1 + 8 + 16 + 4;
const COMMITTED_FLAG_SIZE: usize = 1;
const CRC32_SIZE: usize = 4;
const DEFAULT_ROTATION_SIZE: u64 = 16 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum JournalError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("CRC32 mismatch: expected {expected}, got {actual}")]
    Crc32Mismatch { expected: u32, actual: u32 },
    #[error("invalid record type: {0}")]
    InvalidType(u8),
    #[error("record truncated")]
    Truncated,
}

pub struct BadouJournal {
    file: Mutex<File>,
    path: PathBuf,
    rotation_size: u64,
    current_size: Mutex<u64>,
}

impl BadouJournal {
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

    pub fn append(&self, entry: &BadouJournalEntry) -> Result<u64, JournalError> {
        let encoded = encode_entry(entry)?;
        let mut guard = self.file.lock();
        let mut size_guard = self.current_size.lock();

        if *size_guard + encoded.len() as u64 > self.rotation_size {
            self.rotate_locked(&mut guard, &mut size_guard)?;
        }

        let offset = *size_guard;
        guard.write_all(&encoded)?;
        guard.flush()?;
        *size_guard += encoded.len() as u64;
        Ok(offset)
    }

    pub fn read_from(&self, offset: u64) -> Result<Vec<BadouJournalEntry>, JournalError> {
        let mut guard = self.file.lock();
        guard.seek(SeekFrom::Start(offset))?;

        let mut entries = Vec::new();

        loop {
            match decode_entry(&mut *guard) {
                Ok(Some(entry)) => {
                    entries.push(entry);
                }
                Ok(None) => break,
                Err(JournalError::Crc32Mismatch { .. }) => {
                    tracing::warn!("CRC32 mismatch, skipping remaining records");
                    break;
                }
                Err(JournalError::Truncated) => break,
                Err(e) => return Err(e),
            }
        }

        Ok(entries)
    }

    pub fn read_all(&self) -> Result<Vec<BadouJournalEntry>, JournalError> {
        self.read_from(0)
    }

    fn rotate_locked(
        &self,
        file: &mut File,
        size: &mut u64,
    ) -> Result<(), JournalError> {
        let archive_path = format!(
            "{}.archive.{}",
            self.path.display(),
            Utc::now().timestamp()
        );
        std::fs::rename(&self.path, &archive_path)?;

        *file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&self.path)?;
        *size = 0;
        Ok(())
    }

    pub fn current_size(&self) -> u64 {
        *self.current_size.lock()
    }
}

fn encode_entry(entry: &BadouJournalEntry) -> Result<Vec<u8>, JournalError> {
    let payload = &entry.payload;
    let payload_len = payload.len() as u32;

    let mut buf = Vec::with_capacity(
        RECORD_HEADER_SIZE + payload.len() + COMMITTED_FLAG_SIZE + CRC32_SIZE,
    );

    buf.push(entry.op_type.as_u8());
    buf.extend_from_slice(&entry.timestamp.timestamp_millis().to_le_bytes());
    buf.extend_from_slice(entry.job_id.as_bytes());
    buf.extend_from_slice(&payload_len.to_le_bytes());
    buf.extend_from_slice(payload);
    buf.push(if entry.committed { 1 } else { 0 });

    let crc = crc32(&buf);
    buf.extend_from_slice(&crc.to_le_bytes());

    Ok(buf)
}

fn decode_entry<R: Read>(reader: &mut R) -> Result<Option<BadouJournalEntry>, JournalError> {
    let mut header = [0u8; RECORD_HEADER_SIZE];
    match reader.read_exact(&mut header) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(JournalError::Io(e)),
    }

    let op_type = JournalOpType::from_u8(header[0])
        .ok_or(JournalError::InvalidType(header[0]))?;

    let ts_millis = i64::from_le_bytes([
        header[1], header[2], header[3], header[4],
        header[5], header[6], header[7], header[8],
    ]);
    let timestamp = chrono::DateTime::from_timestamp_millis(ts_millis)
        .unwrap_or_else(Utc::now);

    let job_id_bytes: [u8; 16] = [
        header[9], header[10], header[11], header[12],
        header[13], header[14], header[15], header[16],
        header[17], header[18], header[19], header[20],
        header[21], header[22], header[23], header[24],
    ];
    let job_id = Uuid::from_bytes(job_id_bytes);

    let payload_len = u32::from_le_bytes([
        header[25], header[26], header[27], header[28],
    ]) as usize;

    let mut payload = vec![0u8; payload_len];
    reader.read_exact(&mut payload).map_err(|_| JournalError::Truncated)?;

    let mut committed_buf = [0u8; 1];
    reader.read_exact(&mut committed_buf).map_err(|_| JournalError::Truncated)?;
    let committed = committed_buf[0] == 1;

    let mut crc_buf = [0u8; 4];
    reader.read_exact(&mut crc_buf).map_err(|_| JournalError::Truncated)?;
    let stored_crc = u32::from_le_bytes(crc_buf);

    let mut record_for_crc = Vec::with_capacity(RECORD_HEADER_SIZE + payload_len + COMMITTED_FLAG_SIZE);
    record_for_crc.extend_from_slice(&header);
    record_for_crc.extend_from_slice(&payload);
    record_for_crc.push(committed_buf[0]);
    let computed_crc = crc32(&record_for_crc);

    if stored_crc != computed_crc {
        return Err(JournalError::Crc32Mismatch {
            expected: stored_crc,
            actual: computed_crc,
        });
    }

    Ok(Some(BadouJournalEntry {
        op_type,
        timestamp,
        job_id,
        payload,
        committed,
    }))
}

fn crc32(data: &[u8]) -> u32 {
    let mut hasher = Crc32Hasher::new();
    hasher.update(data);
    hasher.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_and_read() {
        let tmp = tempfile::tempdir().unwrap();
        let journal = BadouJournal::open(tmp.path().join("test.journal")).unwrap();

        let entry = BadouJournalEntry::new(
            JournalOpType::CommitStep,
            Uuid::new_v4(),
            b"test payload".to_vec(),
        );
        journal.append(&entry).unwrap();

        let entries = journal.read_all().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].op_type, JournalOpType::CommitStep);
        assert_eq!(entries[0].payload, b"test payload");
        assert!(!entries[0].committed);
    }

    #[test]
    fn append_multiple_and_read() {
        let tmp = tempfile::tempdir().unwrap();
        let journal = BadouJournal::open(tmp.path().join("multi.journal")).unwrap();

        for i in 0..5 {
            let entry = BadouJournalEntry::new(
                JournalOpType::GcStep,
                Uuid::new_v4(),
                vec![i as u8],
            );
            journal.append(&entry).unwrap();
        }

        let entries = journal.read_all().unwrap();
        assert_eq!(entries.len(), 5);
        for (i, e) in entries.iter().enumerate() {
            assert_eq!(e.payload, vec![i as u8]);
        }
    }

    #[test]
    fn committed_flag_persists() {
        let tmp = tempfile::tempdir().unwrap();
        let journal = BadouJournal::open(tmp.path().join("committed.journal")).unwrap();

        let entry = BadouJournalEntry::new(
            JournalOpType::StateTransition,
            Uuid::new_v4(),
            vec![1, 2, 3],
        ).committed();
        journal.append(&entry).unwrap();

        let entries = journal.read_all().unwrap();
        assert!(entries[0].committed);
    }

    #[test]
    fn rotation_creates_archive() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("rotate.journal");
        let journal = BadouJournal::open_with_rotation(&path, 128).unwrap();

        for _ in 0..10 {
            let entry = BadouJournalEntry::new(
                JournalOpType::CommitStep,
                Uuid::new_v4(),
                vec![0u8; 32],
            );
            journal.append(&entry).unwrap();
        }

        let archives: Vec<_> = std::fs::read_dir(tmp.path()).unwrap()
            .filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().contains("archive"))
            .collect();
        assert!(!archives.is_empty(), "archive file should exist after rotation");
    }

    #[test]
    fn crc32_mismatch_detected() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("corrupt.journal");

        let entry = BadouJournalEntry::new(
            JournalOpType::VerifyStep,
            Uuid::new_v4(),
            b"hello".to_vec(),
        );

        {
            let journal = BadouJournal::open(&path).unwrap();
            journal.append(&entry).unwrap();
        }

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(&path).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        file.write_all(&[99u8]).unwrap();

        let journal = BadouJournal::open(&path).unwrap();
        let result = journal.read_all();
        assert!(result.is_err() || result.unwrap().is_empty());
    }
}
