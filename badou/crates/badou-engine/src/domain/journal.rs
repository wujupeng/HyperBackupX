use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use hbx_core::domain::common::RepositoryId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JournalOp {
    PutChunk,
    DeleteChunk,
    CommitSnapshot,
    DeleteVersion,
    Gc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Journal {
    pub journal_id: Uuid,
    pub repo_id: RepositoryId,
    pub operation: JournalOp,
    pub payload: Vec<u8>,
    pub timestamp: DateTime<Utc>,
    pub committed: bool,
    pub append_only: bool,
}

impl Journal {
    pub fn new(repo_id: RepositoryId, operation: JournalOp, payload: Vec<u8>) -> Self {
        Self {
            journal_id: Uuid::new_v4(),
            repo_id,
            operation,
            payload,
            timestamp: Utc::now(),
            committed: false,
            append_only: true,
        }
    }

    pub fn commit(&mut self) {
        self.committed = true;
    }

    pub fn is_committed(&self) -> bool {
        self.committed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn journal_new_uncommitted() {
        let j = Journal::new(
            RepositoryId(Uuid::new_v4()),
            JournalOp::PutChunk,
            vec![1, 2, 3],
        );
        assert!(!j.is_committed());
        assert!(j.append_only);
    }

    #[test]
    fn journal_commit() {
        let mut j = Journal::new(
            RepositoryId(Uuid::new_v4()),
            JournalOp::CommitSnapshot,
            vec![],
        );
        j.commit();
        assert!(j.is_committed());
    }
}