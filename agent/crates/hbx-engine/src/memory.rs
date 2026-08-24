use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::Notify;

pub struct MemoryBudget {
    limit: u64,
    used: AtomicU64,
    notify: Notify,
}

pub struct MemoryGuard {
    budget: Arc<MemoryBudget>,
    bytes: u64,
}

impl MemoryBudget {
    pub fn new(limit: u64) -> Arc<Self> {
        Arc::new(Self {
            limit,
            used: AtomicU64::new(0),
            notify: Notify::new(),
        })
    }

    pub async fn acquire(self: &Arc<Self>, bytes: u64) -> MemoryGuard {
        if bytes == 0 {
            return MemoryGuard {
                budget: Arc::clone(self),
                bytes: 0,
            };
        }
        loop {
            let current = self.used.load(Ordering::Acquire);
            if current + bytes <= self.limit {
                match self.used.compare_exchange(
                    current,
                    current + bytes,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        return MemoryGuard {
                            budget: Arc::clone(self),
                            bytes,
                        };
                    }
                    Err(_) => continue,
                }
            }
            self.notify.notified().await;
        }
    }

    pub fn used(&self) -> u64 {
        self.used.load(Ordering::Relaxed)
    }

    pub fn limit(&self) -> u64 {
        self.limit
    }

    fn release(&self, bytes: u64) {
        if bytes == 0 {
            return;
        }
        self.used.fetch_sub(bytes, Ordering::Release);
        self.notify.notify_one();
    }
}

impl Drop for MemoryGuard {
    fn drop(&mut self) {
        self.budget.release(self.bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_acquire_release() {
        let budget = MemoryBudget::new(100);
        let guard = budget.acquire(50).await;
        assert_eq!(budget.used(), 50);
        drop(guard);
        assert_eq!(budget.used(), 0);
    }

    #[tokio::test]
    async fn test_backpressure() {
        let budget = MemoryBudget::new(100);
        let guard1 = budget.acquire(80).await;
        assert_eq!(budget.used(), 80);

        let budget2 = Arc::clone(&budget);
        let handle = tokio::spawn(async move {
            let guard = budget2.acquire(50).await;
            assert_eq!(budget2.used(), 50);
            drop(guard);
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        drop(guard1);

        handle.await.unwrap();
        assert_eq!(budget.used(), 0);
    }

    #[tokio::test]
    async fn test_zero_bytes() {
        let budget = MemoryBudget::new(100);
        let guard = budget.acquire(0).await;
        assert_eq!(budget.used(), 0);
        drop(guard);
    }
}