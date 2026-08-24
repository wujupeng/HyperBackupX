use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

#[derive(Debug, Clone)]
pub struct TokenBucket {
    capacity: Arc<AtomicU64>,
    refill_rate: Arc<AtomicU64>,
    state: Arc<Mutex<BucketState>>,
}

#[derive(Debug)]
struct BucketState {
    tokens: f64,
    last_refill: Instant,
}

impl TokenBucket {
    pub fn new(capacity: u64, refill_rate_per_sec: u64) -> Self {
        Self {
            capacity: Arc::new(AtomicU64::new(capacity)),
            refill_rate: Arc::new(AtomicU64::new(refill_rate_per_sec)),
            state: Arc::new(Mutex::new(BucketState {
                tokens: capacity as f64,
                last_refill: Instant::now(),
            })),
        }
    }

    pub fn set_rate(&self, refill_rate_per_sec: u64) {
        self.refill_rate.store(refill_rate_per_sec, AtomicOrdering::Relaxed);
    }

    pub fn set_capacity(&self, capacity: u64) {
        self.capacity.store(capacity, AtomicOrdering::Relaxed);
    }

    pub fn current_rate(&self) -> u64 {
        self.refill_rate.load(AtomicOrdering::Relaxed)
    }

    fn refill(&self) {
        let rate = self.refill_rate.load(AtomicOrdering::Relaxed) as f64;
        let cap = self.capacity.load(AtomicOrdering::Relaxed) as f64;
        let mut state = self.state.lock();
        let now = Instant::now();
        let elapsed = now.duration_since(state.last_refill).as_secs_f64();
        state.tokens = (state.tokens + rate * elapsed).min(cap);
        state.last_refill = now;
    }

    pub fn try_acquire(&self, bytes: u64) -> bool {
        self.refill();
        let mut state = self.state.lock();
        if state.tokens >= bytes as f64 {
            state.tokens -= bytes as f64;
            true
        } else {
            false
        }
    }

    pub async fn acquire(&self, bytes: u64) {
        loop {
            self.refill();
            let wait_time = {
                let mut state = self.state.lock();
                if state.tokens >= bytes as f64 {
                    state.tokens -= bytes as f64;
                    return;
                }
                let rate = self.refill_rate.load(AtomicOrdering::Relaxed) as f64;
                if rate <= 0.0 {
                    return;
                }
                let needed = bytes as f64 - state.tokens;
                Some(Duration::from_secs_f64(needed / rate))
            };

            if let Some(d) = wait_time {
                sleep(d).await;
            }
        }
    }

    pub fn available_tokens(&self) -> u64 {
        self.refill();
        let state = self.state.lock();
        state.tokens as u64
    }
}

#[derive(Clone)]
pub struct RateLimiter {
    upload: TokenBucket,
    download: TokenBucket,
    disk_read: Option<TokenBucket>,
    disk_write: Option<TokenBucket>,
}

impl RateLimiter {
    pub fn new(
        upload_bytes_per_sec: u64,
        download_bytes_per_sec: u64,
    ) -> Self {
        let upload_cap = upload_bytes_per_sec.max(1) * 2;
        let dl_cap = download_bytes_per_sec.max(1) * 2;
        Self {
            upload: TokenBucket::new(upload_cap, upload_bytes_per_sec),
            download: TokenBucket::new(dl_cap, download_bytes_per_sec),
            disk_read: None,
            disk_write: None,
        }
    }

    pub fn with_disk_limits(
        mut self,
        disk_read_bytes_per_sec: u64,
        disk_write_bytes_per_sec: u64,
    ) -> Self {
        let r_cap = disk_read_bytes_per_sec.max(1) * 2;
        let w_cap = disk_write_bytes_per_sec.max(1) * 2;
        self.disk_read = Some(TokenBucket::new(r_cap, disk_read_bytes_per_sec));
        self.disk_write = Some(TokenBucket::new(w_cap, disk_write_bytes_per_sec));
        self
    }

    pub fn set_upload_rate(&self, bytes_per_sec: u64) {
        self.upload.set_rate(bytes_per_sec);
        self.upload.set_capacity(bytes_per_sec.max(1) * 2);
    }

    pub fn set_download_rate(&self, bytes_per_sec: u64) {
        self.download.set_rate(bytes_per_sec);
        self.download.set_capacity(bytes_per_sec.max(1) * 2);
    }

    pub async fn acquire_upload(&self, bytes: u64) {
        self.upload.acquire(bytes).await;
    }

    pub async fn acquire_download(&self, bytes: u64) {
        self.download.acquire(bytes).await;
    }

    pub async fn acquire_disk_read(&self, bytes: u64) {
        if let Some(ref bucket) = self.disk_read {
            bucket.acquire(bytes).await;
        }
    }

    pub async fn acquire_disk_write(&self, bytes: u64) {
        if let Some(ref bucket) = self.disk_write {
            bucket.acquire(bytes).await;
        }
    }

    pub fn try_acquire_upload(&self, bytes: u64) -> bool {
        self.upload.try_acquire(bytes)
    }

    pub fn try_acquire_download(&self, bytes: u64) -> bool {
        self.download.try_acquire(bytes)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub backoff_multiplier: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff_ms: 1000,
            max_backoff_ms: 60000,
            backoff_multiplier: 2.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetryDecision {
    Retry { attempt: u32, backoff: Duration },
    GiveUp,
}

#[derive(Debug, Clone)]
pub struct RetryState {
    policy: RetryPolicy,
    current_attempt: u32,
}

impl RetryState {
    pub fn new(policy: RetryPolicy) -> Self {
        Self {
            policy,
            current_attempt: 0,
        }
    }

    pub fn current_attempt(&self) -> u32 {
        self.current_attempt
    }

    pub fn on_failure(&mut self) -> RetryDecision {
        self.current_attempt += 1;
        if self.current_attempt > self.policy.max_retries {
            RetryDecision::GiveUp
        } else {
            let backoff_ms = (self.policy.initial_backoff_ms as f64
                * self.policy.backoff_multiplier.powi((self.current_attempt - 1) as i32))
                .min(self.policy.max_backoff_ms as f64);
            RetryDecision::Retry {
                attempt: self.current_attempt,
                backoff: Duration::from_millis(backoff_ms as u64),
            }
        }
    }

    pub fn reset(&mut self) {
        self.current_attempt = 0;
    }

    pub fn is_exhausted(&self) -> bool {
        self.current_attempt > self.policy.max_retries
    }
}

pub async fn execute_with_retry<F, Fut, T, E>(
    policy: RetryPolicy,
    mut operation: F,
) -> Result<T, E>
where
    F: FnMut(u32) -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let mut state = RetryState::new(policy);
    loop {
        let attempt = state.current_attempt();
        match operation(attempt).await {
            Ok(result) => return Ok(result),
            Err(e) => match state.on_failure() {
                RetryDecision::GiveUp => return Err(e),
                RetryDecision::Retry { backoff, .. } => {
                    sleep(backoff).await;
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket_initial_full() {
        let bucket = TokenBucket::new(1000, 100);
        assert!(bucket.try_acquire(1000));
        assert!(!bucket.try_acquire(1));
    }

    #[test]
    fn test_token_bucket_refill() {
        let bucket = TokenBucket::new(1000, 1000);
        assert!(bucket.try_acquire(1000));
        assert!(!bucket.try_acquire(1));

        std::thread::sleep(Duration::from_millis(100));
        assert!(bucket.try_acquire(50));
    }

    #[test]
    fn test_token_bucket_hot_update_rate() {
        let bucket = TokenBucket::new(10000, 100);
        assert!(bucket.try_acquire(5000));

        bucket.set_rate(10000);
        assert_eq!(bucket.current_rate(), 10000);

        std::thread::sleep(Duration::from_millis(50));
        let avail = bucket.available_tokens();
        assert!(avail > 5000);
    }

    #[test]
    fn test_token_bucket_partial_acquire() {
        let bucket = TokenBucket::new(500, 100);
        assert!(bucket.try_acquire(300));
        assert!(bucket.try_acquire(200));
        assert!(!bucket.try_acquire(1));
    }

    #[tokio::test]
    async fn test_token_bucket_async_acquire() {
        let bucket = TokenBucket::new(100, 1000);
        bucket.acquire(100).await;
        let start = Instant::now();
        bucket.acquire(100).await;
        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(80));
    }

    #[test]
    fn test_rate_limiter_upload_download() {
        let limiter = RateLimiter::new(1000, 2000);
        assert!(limiter.try_acquire_upload(500));
        assert!(limiter.try_acquire_download(1000));
    }

    #[test]
    fn test_rate_limiter_hot_update() {
        let limiter = RateLimiter::new(1000, 2000);
        limiter.set_upload_rate(5000);
        limiter.set_download_rate(10000);

        std::thread::sleep(Duration::from_millis(10));
        assert!(limiter.try_acquire_upload(1000));
    }

    #[tokio::test]
    async fn test_rate_limiter_with_disk_limits() {
        let limiter = RateLimiter::new(1000, 1000)
            .with_disk_limits(500, 500);

        limiter.acquire_disk_read(100).await;
        limiter.acquire_disk_write(100).await;
    }

    #[test]
    fn test_retry_policy_default() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_retries, 3);
        assert_eq!(policy.initial_backoff_ms, 1000);
        assert_eq!(policy.max_backoff_ms, 60000);
        assert_eq!(policy.backoff_multiplier, 2.0);
    }

    #[test]
    fn test_retry_state_first_failure() {
        let mut state = RetryState::new(RetryPolicy::default());
        let decision = state.on_failure();
        match decision {
            RetryDecision::Retry { attempt, backoff } => {
                assert_eq!(attempt, 1);
                assert_eq!(backoff, Duration::from_millis(1000));
            }
            RetryDecision::GiveUp => panic!("should retry"),
        }
    }

    #[test]
    fn test_retry_state_exponential_backoff() {
        let mut state = RetryState::new(RetryPolicy::default());
        let d1 = state.on_failure();
        let d2 = state.on_failure();
        let d3 = state.on_failure();
        let d4 = state.on_failure();

        match d1 {
            RetryDecision::Retry { backoff, .. } => assert_eq!(backoff, Duration::from_millis(1000)),
            _ => panic!(),
        }
        match d2 {
            RetryDecision::Retry { backoff, .. } => assert_eq!(backoff, Duration::from_millis(2000)),
            _ => panic!(),
        }
        match d3 {
            RetryDecision::Retry { backoff, .. } => assert_eq!(backoff, Duration::from_millis(4000)),
            _ => panic!(),
        }
        assert_eq!(d4, RetryDecision::GiveUp);
    }

    #[test]
    fn test_retry_state_max_backoff_cap() {
        let policy = RetryPolicy {
            max_retries: 10,
            initial_backoff_ms: 1000,
            max_backoff_ms: 5000,
            backoff_multiplier: 2.0,
        };
        let mut state = RetryState::new(policy);
        for _ in 0..10 {
            match state.on_failure() {
                RetryDecision::Retry { backoff, .. } => {
                    assert!(backoff <= Duration::from_millis(5000));
                }
                RetryDecision::GiveUp => break,
            }
        }
    }

    #[test]
    fn test_retry_state_reset() {
        let mut state = RetryState::new(RetryPolicy::default());
        state.on_failure();
        state.on_failure();
        assert_eq!(state.current_attempt(), 2);

        state.reset();
        assert_eq!(state.current_attempt(), 0);
        assert!(!state.is_exhausted());
    }

    #[test]
    fn test_retry_state_is_exhausted() {
        let mut state = RetryState::new(RetryPolicy {
            max_retries: 2,
            initial_backoff_ms: 100,
            max_backoff_ms: 1000,
            backoff_multiplier: 2.0,
        });
        assert!(!state.is_exhausted());
        state.on_failure();
        assert!(!state.is_exhausted());
        state.on_failure();
        assert!(!state.is_exhausted());
        state.on_failure();
        assert!(state.is_exhausted());
    }

    #[tokio::test]
    async fn test_execute_with_retry_success_first_try() {
        let result: Result<i32, &str> = execute_with_retry(RetryPolicy::default(), |_| async {
            Ok(42)
        })
        .await;
        assert_eq!(result, Ok(42));
    }

    #[tokio::test]
    async fn test_execute_with_retry_success_after_retries() {
        let counter = Arc::new(AtomicU64::new(0));
        let counter_clone = counter.clone();
        let result: Result<i32, &str> = execute_with_retry(
            RetryPolicy {
                max_retries: 3,
                initial_backoff_ms: 1,
                max_backoff_ms: 10,
                backoff_multiplier: 2.0,
            },
            |_| {
                let c = counter_clone.clone();
                async move {
                    let n = c.fetch_add(1, AtomicOrdering::Relaxed);
                    if n < 2 {
                        Err("fail")
                    } else {
                        Ok(42)
                    }
                }
            },
        )
        .await;
        assert_eq!(result, Ok(42));
        assert_eq!(counter.load(AtomicOrdering::Relaxed), 3);
    }

    #[tokio::test]
    async fn test_execute_with_retry_give_up() {
        let policy = RetryPolicy {
            max_retries: 2,
            initial_backoff_ms: 1,
            max_backoff_ms: 10,
            backoff_multiplier: 2.0,
        };
        let result: Result<i32, &str> = execute_with_retry(policy, |_| async {
            Err::<i32, &str>("always fails")
        })
        .await;
        assert_eq!(result, Err("always fails"));
    }
}