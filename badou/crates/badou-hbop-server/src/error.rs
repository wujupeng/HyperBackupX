//! 统一错误码映射：业务异常 → HbopErrorCode + tonic::Status。
//!
//! 映射 design.md §2.2.2.7 错误码体系。

use badou_proto::HbopErrorCode;
use tonic::Status;

/// 将 HbopErrorCode 转为 tonic::Status。
pub fn to_status(code: HbopErrorCode, message: impl Into<String>) -> Status {
    let msg = message.into();
    match code {
        HbopErrorCode::AuthFailed => Status::unauthenticated(format!("AUTH_FAILED: {}", msg)),
        HbopErrorCode::PermissionDenied => Status::permission_denied(format!("FORBIDDEN: {}", msg)),
        HbopErrorCode::VersionMismatch => Status::invalid_argument(format!("VERSION_MISMATCH: {}", msg)),
        HbopErrorCode::RepoNotFound => Status::not_found(format!("REPO_NOT_FOUND: {}", msg)),
        HbopErrorCode::RepoAlreadyExists => Status::already_exists(format!("REPO_EXISTS: {}", msg)),
        HbopErrorCode::VersionNotFound => Status::not_found(format!("VERSION_NOT_FOUND: {}", msg)),
        HbopErrorCode::SnapshotNotFound => Status::not_found(format!("SNAPSHOT_NOT_FOUND: {}", msg)),
        HbopErrorCode::ChunkNotFound => Status::not_found(format!("CHUNK_NOT_FOUND: {}", msg)),
        HbopErrorCode::ManifestNotFound => Status::not_found(format!("MANIFEST_NOT_FOUND: {}", msg)),
        HbopErrorCode::ImmutableConflict => Status::failed_precondition(format!("IMMUTABLE_CONFLICT: {}", msg)),
        HbopErrorCode::StateConflict => Status::failed_precondition(format!("STATE_CONFLICT: {}", msg)),
        HbopErrorCode::HashMismatch => Status::failed_precondition(format!("HASH_MISMATCH: {}", msg)),
        HbopErrorCode::CorruptedData => Status::data_loss(format!("SNAPSHOT_CORRUPTED: {}", msg)),
        HbopErrorCode::DiskFull => Status::resource_exhausted(format!("REPO_FULL: {}", msg)),
        HbopErrorCode::InternalError => Status::internal(format!("INTERNAL_ERROR: {}", msg)),
        HbopErrorCode::RateLimited => Status::resource_exhausted(format!("RATE_LIMITED: {}", msg)),
        HbopErrorCode::HbopErrorUnspecified => Status::internal(format!("INTERNAL_ERROR: {}", msg)),
    }
}

/// RepositoryError → Status
pub fn repo_error_to_status(e: &badou_ops::RepositoryError) -> Status {
    use badou_ops::RepositoryError;
    match e {
        RepositoryError::AlreadyExists(_) => to_status(HbopErrorCode::RepoAlreadyExists, e.to_string()),
        RepositoryError::NotFound(_) => to_status(HbopErrorCode::RepoNotFound, e.to_string()),
        RepositoryError::Immutable(_) => to_status(HbopErrorCode::ImmutableConflict, e.to_string()),
        other => to_status(HbopErrorCode::InternalError, other.to_string()),
    }
}

/// ChunkOpsError → Status
pub fn chunk_error_to_status(e: &badou_ops::ChunkOpsError) -> Status {
    use badou_ops::ChunkOpsError;
    match e {
        ChunkOpsError::NotFound(_) => to_status(HbopErrorCode::ChunkNotFound, e.to_string()),
        ChunkOpsError::Referenced(_) => to_status(HbopErrorCode::ImmutableConflict, e.to_string()),
        other => to_status(HbopErrorCode::InternalError, other.to_string()),
    }
}

/// SnapshotOpsError → Status
pub fn snapshot_error_to_status(e: &badou_ops::SnapshotOpsError) -> Status {
    use badou_ops::SnapshotOpsError;
    match e {
        SnapshotOpsError::NotFound(_) => to_status(HbopErrorCode::SnapshotNotFound, e.to_string()),
        other => to_status(HbopErrorCode::InternalError, other.to_string()),
    }
}

/// VersionOpsError → Status
pub fn version_error_to_status(e: &badou_ops::VersionOpsError) -> Status {
    use badou_ops::VersionOpsError;
    match e {
        VersionOpsError::NotFound(_) => to_status(HbopErrorCode::VersionNotFound, e.to_string()),
        VersionOpsError::InvalidTransition(a, b) => {
            to_status(HbopErrorCode::StateConflict, format!("{:?} -> {:?}", a, b))
        }
        VersionOpsError::ParentNotFound(_) => to_status(HbopErrorCode::VersionNotFound, e.to_string()),
        other => to_status(HbopErrorCode::InternalError, other.to_string()),
    }
}

/// CommitError → Status
pub fn commit_error_to_status(e: &badou_ops::CommitError) -> Status {
    use badou_ops::CommitError;
    match e {
        CommitError::VerificationFailed(_) => to_status(HbopErrorCode::CorruptedData, e.to_string()),
        CommitError::ChunkStore(badou_store::ChunkStoreError::NotFound(_)) => {
            to_status(HbopErrorCode::ChunkNotFound, e.to_string())
        }
        other => to_status(HbopErrorCode::InternalError, other.to_string()),
    }
}

/// VerifyError → Status
pub fn verify_error_to_status(e: &badou_verify::VerifyError) -> Status {
    use badou_verify::VerifyError;
    match e {
        VerifyError::VersionNotFound(_) => to_status(HbopErrorCode::VersionNotFound, e.to_string()),
        other => to_status(HbopErrorCode::InternalError, other.to_string()),
    }
}

/// RecoveryError → Status
pub fn recovery_error_to_status(e: &badou_recovery::RecoveryError) -> Status {
    use badou_recovery::RecoveryError;
    match e {
        RecoveryError::SnapshotNotFound(_) => to_status(HbopErrorCode::SnapshotNotFound, e.to_string()),
        RecoveryError::SnapshotCorrupted(_) => to_status(HbopErrorCode::CorruptedData, e.to_string()),
        RecoveryError::NotSealed(_) => to_status(HbopErrorCode::StateConflict, e.to_string()),
        RecoveryError::ChunkNotFound(_) => to_status(HbopErrorCode::ChunkNotFound, e.to_string()),
        RecoveryError::HashMismatch { .. } => to_status(HbopErrorCode::HashMismatch, e.to_string()),
        other => to_status(HbopErrorCode::InternalError, other.to_string()),
    }
}

/// GcExecutorError → Status
pub fn gc_error_to_status(e: &badou_gc::GcExecutorError) -> Status {
    to_status(HbopErrorCode::InternalError, e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_code_mapping_covers_all() {
        let codes = [
            HbopErrorCode::AuthFailed,
            HbopErrorCode::PermissionDenied,
            HbopErrorCode::VersionMismatch,
            HbopErrorCode::RepoNotFound,
            HbopErrorCode::RepoAlreadyExists,
            HbopErrorCode::VersionNotFound,
            HbopErrorCode::SnapshotNotFound,
            HbopErrorCode::ChunkNotFound,
            HbopErrorCode::ManifestNotFound,
            HbopErrorCode::ImmutableConflict,
            HbopErrorCode::StateConflict,
            HbopErrorCode::HashMismatch,
            HbopErrorCode::CorruptedData,
            HbopErrorCode::DiskFull,
            HbopErrorCode::InternalError,
            HbopErrorCode::RateLimited,
            HbopErrorCode::HbopErrorUnspecified,
        ];
        for code in codes {
            let status = to_status(code, "test");
            assert!(status.code() != tonic::Code::Ok);
        }
    }
}