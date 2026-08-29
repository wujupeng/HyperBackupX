use chrono::{DateTime, Utc};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ImmutableConflict {
    #[error("repository {0} is immutable until {1}")]
    RepoImmutable(Uuid, DateTime<Utc>),
    #[error("version {0} is immutable until {1}")]
    VersionImmutable(Uuid, DateTime<Utc>),
}

pub struct ImmutableGuard;

impl ImmutableGuard {
    pub fn check(
        immutable_until: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
        id: Uuid,
        is_version: bool,
    ) -> Result<(), ImmutableConflict> {
        if let Some(until) = immutable_until {
            if now < until {
                return Err(if is_version {
                    ImmutableConflict::VersionImmutable(id, until)
                } else {
                    ImmutableConflict::RepoImmutable(id, until)
                });
            }
        }
        Ok(())
    }

    pub fn check_repo(
        immutable_until: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
        repo_id: Uuid,
    ) -> Result<(), ImmutableConflict> {
        Self::check(immutable_until, now, repo_id, false)
    }

    pub fn check_version(
        immutable_until: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
        version_id: Uuid,
    ) -> Result<(), ImmutableConflict> {
        Self::check(immutable_until, now, version_id, true)
    }

    pub fn set_immutable(until: DateTime<Utc>) -> Option<DateTime<Utc>> {
        Some(until)
    }

    pub fn clear_immutable() -> Option<DateTime<Utc>> {
        None
    }

    pub fn is_expired(
        immutable_until: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
    ) -> bool {
        immutable_until.map(|t| now >= t).unwrap_or(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn immutable_blocks_deletion() {
        let now = Utc::now();
        let until = now + chrono::Duration::hours(24);
        let repo_id = Uuid::new_v4();
        let result = ImmutableGuard::check_repo(Some(until), now, repo_id);
        assert!(result.is_err());
    }

    #[test]
    fn expired_allows_deletion() {
        let now = Utc::now();
        let until = now - chrono::Duration::hours(1);
        let repo_id = Uuid::new_v4();
        let result = ImmutableGuard::check_repo(Some(until), now, repo_id);
        assert!(result.is_ok());
    }

    #[test]
    fn no_immutable_allows_deletion() {
        let now = Utc::now();
        let repo_id = Uuid::new_v4();
        let result = ImmutableGuard::check_repo(None, now, repo_id);
        assert!(result.is_ok());
    }

    #[test]
    fn version_immutable_blocks() {
        let now = Utc::now();
        let until = now + chrono::Duration::days(7);
        let version_id = Uuid::new_v4();
        let result = ImmutableGuard::check_version(Some(until), now, version_id);
        assert!(result.is_err());
        match result {
            Err(ImmutableConflict::VersionImmutable(id, _)) => assert_eq!(id, version_id),
            _ => panic!("expected VersionImmutable"),
        }
    }

    #[test]
    fn admin_cannot_bypass_immutable() {
        let now = Utc::now();
        let until = now + chrono::Duration::days(30);
        let repo_id = Uuid::new_v4();
        let result = ImmutableGuard::check_repo(Some(until), now, repo_id);
        assert!(result.is_err());
    }

    #[test]
    fn is_expired_checks() {
        let now = Utc::now();
        let future = now + chrono::Duration::hours(1);
        let past = now - chrono::Duration::hours(1);
        assert!(!ImmutableGuard::is_expired(Some(future), now));
        assert!(ImmutableGuard::is_expired(Some(past), now));
        assert!(ImmutableGuard::is_expired(None, now));
    }
}