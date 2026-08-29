use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JournalOpType {
    CommitStep = 1,
    GcStep = 2,
    VerifyStep = 3,
    StateTransition = 4,
    Recovery = 5,
}

impl JournalOpType {
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::CommitStep),
            2 => Some(Self::GcStep),
            3 => Some(Self::VerifyStep),
            4 => Some(Self::StateTransition),
            5 => Some(Self::Recovery),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BadouJournalEntry {
    pub op_type: JournalOpType,
    pub timestamp: DateTime<Utc>,
    pub job_id: Uuid,
    pub payload: Vec<u8>,
    pub committed: bool,
}

impl BadouJournalEntry {
    pub fn new(op_type: JournalOpType, job_id: Uuid, payload: Vec<u8>) -> Self {
        Self {
            op_type,
            timestamp: Utc::now(),
            job_id,
            payload,
            committed: false,
        }
    }

    pub fn committed(mut self) -> Self {
        self.committed = true;
        self
    }
}