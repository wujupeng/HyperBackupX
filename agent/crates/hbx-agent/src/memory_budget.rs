//! 内存预算控制（空闲 ≤40MB，单任务 ≤120MB）
//!
//! 使用 sysinfo 获取进程内存，超预算时自动缩减热缓存和 Journal 索引。

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 内存预算配置
#[derive(Debug, Clone)]
pub struct MemoryBudget {
    /// 空闲时最大内存（字节），默认 40MB
    pub max_idle_bytes: u64,
    /// 单任务最大内存（字节），默认 120MB
    pub max_task_bytes: u64,
    /// 热缓存预算（字节），默认 16MB
    pub cache_budget_bytes: u64,
    /// Journal 索引预算（字节），默认 8MB
    pub journal_index_budget_bytes: u64,
}

impl Default for MemoryBudget {
    fn default() -> Self {
        Self {
            max_idle_bytes: 40 * 1024 * 1024,
            max_task_bytes: 120 * 1024 * 1024,
            cache_budget_bytes: 16 * 1024 * 1024,
            journal_index_budget_bytes: 8 * 1024 * 1024,
        }
    }
}

impl MemoryBudget {
    /// 创建严格的 40MB 空闲预算
    pub fn strict_40mb() -> Self {
        Self::default()
    }

    /// 创建宽松的预算（用于测试）
    pub fn relaxed() -> Self {
        Self {
            max_idle_bytes: 512 * 1024 * 1024,
            max_task_bytes: 1024 * 1024 * 1024,
            cache_budget_bytes: 64 * 1024 * 1024,
            journal_index_budget_bytes: 32 * 1024 * 1024,
        }
    }
}

/// 内存使用快照
#[derive(Debug, Clone)]
pub struct MemorySnapshot {
    /// 进程 RSS（常驻集大小），字节
    pub rss_bytes: u64,
    /// 进程 VMS（虚拟内存大小），字节
    pub vms_bytes: u64,
    /// 系统可用内存，字节
    pub available_bytes: u64,
    /// 采集时间
    pub timestamp: Instant,
}

impl MemorySnapshot {
    /// 获取当前内存快照
    pub fn capture() -> Self {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

        let mut sys = System::new();
        sys.refresh_memory();

        let pid = sysinfo::get_current_pid().unwrap_or(sysinfo::Pid::from(0));
        sys.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            true,
            ProcessRefreshKind::everything(),
        );

        let (rss, vms) = if let Some(proc_info) = sys.process(pid) {
            (proc_info.memory(), proc_info.virtual_memory())
        } else {
            (0, 0)
        };

        Self {
            rss_bytes: rss,
            vms_bytes: vms,
            available_bytes: sys.available_memory(),
            timestamp: Instant::now(),
        }
    }

    /// 是否超过空闲预算
    pub fn exceeds_idle(&self, budget: &MemoryBudget) -> bool {
        self.rss_bytes > budget.max_idle_bytes
    }

    /// 是否超过任务预算
    pub fn exceeds_task(&self, budget: &MemoryBudget) -> bool {
        self.rss_bytes > budget.max_task_bytes
    }
}

/// 内存预算执行器
pub struct MemoryBudgetEnforcer {
    budget: MemoryBudget,
    current_cache_usage: Arc<AtomicU64>,
    current_journal_usage: Arc<AtomicU64>,
    check_interval: Duration,
    last_check: std::sync::Mutex<Instant>,
    over_budget_count: AtomicUsize,
}

impl MemoryBudgetEnforcer {
    /// 创建预算执行器
    pub fn new(budget: MemoryBudget) -> Self {
        Self {
            budget,
            current_cache_usage: Arc::new(AtomicU64::new(0)),
            current_journal_usage: Arc::new(AtomicU64::new(0)),
            check_interval: Duration::from_secs(30),
            last_check: std::sync::Mutex::new(Instant::now()),
            over_budget_count: AtomicUsize::new(0),
        }
    }

    /// 获取预算配置引用
    pub fn budget(&self) -> &MemoryBudget {
        &self.budget
    }

    /// 记入缓存使用量
    pub fn add_cache_usage(&self, bytes: u64) {
        self.current_cache_usage.fetch_add(bytes, Ordering::SeqCst);
    }

    /// 减去缓存使用量
    pub fn release_cache_usage(&self, bytes: u64) {
        self.current_cache_usage.fetch_sub(bytes, Ordering::SeqCst);
    }

    /// 计入 Journal 索引使用量
    pub fn add_journal_usage(&self, bytes: u64) {
        self.current_journal_usage.fetch_add(bytes, Ordering::SeqCst);
    }

    /// 减去 Journal 索引使用量
    pub fn release_journal_usage(&self, bytes: u64) {
        self.current_journal_usage.fetch_sub(bytes, Ordering::SeqCst);
    }

    /// 检查并执行预算限制
    ///
    /// 返回执行的缩减操作列表
    pub fn enforce(&self) -> Vec<BudgetAction> {
        let mut actions = Vec::new();

        let should_check = {
            let last = self.last_check.lock().unwrap();
            last.elapsed() >= self.check_interval
        };

        if !should_check {
            return actions;
        }

        {
            let mut last = self.last_check.lock().unwrap();
            *last = Instant::now();
        }

        let snapshot = MemorySnapshot::capture();

        if snapshot.exceeds_idle(&self.budget) {
            self.over_budget_count.fetch_add(1, Ordering::SeqCst);

            let cache_usage = self.current_cache_usage.load(Ordering::SeqCst);
            if cache_usage > self.budget.cache_budget_bytes / 2 {
                let to_free = cache_usage - self.budget.cache_budget_bytes / 2;
                actions.push(BudgetAction::ShrinkCache(to_free));
                self.current_cache_usage
                    .fetch_sub(to_free, Ordering::SeqCst);
            }

            let journal_usage = self.current_journal_usage.load(Ordering::SeqCst);
            if journal_usage > self.budget.journal_index_budget_bytes / 2 {
                let to_free = journal_usage - self.budget.journal_index_budget_bytes / 2;
                actions.push(BudgetAction::CompactJournalIndex(to_free));
                self.current_journal_usage
                    .fetch_sub(to_free, Ordering::SeqCst);
            }

            actions.push(BudgetAction::GcRun);
        }

        actions
    }

    /// 强制执行全部缩减（不考虑间隔）
    pub fn force_shrink(&self) -> Vec<BudgetAction> {
        let mut actions = Vec::new();

        let cache_usage = self.current_cache_usage.load(Ordering::SeqCst);
        if cache_usage > 0 {
            let to_free = cache_usage / 2;
            actions.push(BudgetAction::ShrinkCache(to_free));
            self.current_cache_usage
                .fetch_sub(to_free, Ordering::SeqCst);
        }

        let journal_usage = self.current_journal_usage.load(Ordering::SeqCst);
        if journal_usage > 0 {
            let to_free = journal_usage / 2;
            actions.push(BudgetAction::CompactJournalIndex(to_free));
            self.current_journal_usage
                .fetch_sub(to_free, Ordering::SeqCst);
        }

        actions.push(BudgetAction::GcRun);
        actions
    }

    /// 超预算次数
    pub fn over_budget_count(&self) -> usize {
        self.over_budget_count.load(Ordering::SeqCst)
    }

    /// 当前缓存使用量
    pub fn cache_usage(&self) -> u64 {
        self.current_cache_usage.load(Ordering::SeqCst)
    }

    /// 当前 Journal 索引使用量
    pub fn journal_usage(&self) -> u64 {
        self.current_journal_usage.load(Ordering::SeqCst)
    }
}

/// 预算执行操作
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetAction {
    /// 缩减热缓存（字节数）
    ShrinkCache(u64),
    /// 压缩 Journal 索引（字节数）
    CompactJournalIndex(u64),
    /// 触发 GC
    GcRun,
}

/// LRU 缓存预算控制器
pub struct CacheBudget<K: Eq + std::hash::Hash + Clone> {
    max_bytes: u64,
    current_bytes: u64,
    entries: std::collections::HashMap<K, (Vec<u8>, Instant)>,
    entry_order: std::collections::VecDeque<K>,
}

impl<K: Eq + std::hash::Hash + Clone> CacheBudget<K> {
    /// 创建指定预算的缓存
    pub fn new(max_bytes: u64) -> Self {
        Self {
            max_bytes,
            current_bytes: 0,
            entries: std::collections::HashMap::new(),
            entry_order: std::collections::VecDeque::new(),
        }
    }

    /// 插入条目，自动驱逐最旧的条目以保持在预算内
    pub fn insert(&mut self, key: K, value: Vec<u8>) -> bool {
        let entry_size = value.len() as u64;

        if let Some((old_val, _)) = self.entries.get(&key) {
            self.current_bytes -= old_val.len() as u64;
            self.entry_order.retain(|k| k != &key);
        }

        while self.current_bytes + entry_size > self.max_bytes && !self.entries.is_empty() {
            self.evict_oldest();
        }

        if entry_size > self.max_bytes {
            return false;
        }

        self.current_bytes += entry_size;
        self.entries.insert(key.clone(), (value, Instant::now()));
        self.entry_order.push_back(key);
        true
    }

    /// 获取条目
    pub fn get(&mut self, key: &K) -> Option<&Vec<u8>> {
        if let Some((val, _)) = self.entries.get(key) {
            if let Some(pos) = self.entry_order.iter().position(|k| k == key) {
                let k = self.entry_order.remove(pos).unwrap();
                self.entry_order.push_back(k);
            }
            return Some(val);
        }
        None
    }

    /// 驱逐最旧条目
    fn evict_oldest(&mut self) {
        if let Some(key) = self.entry_order.pop_front() {
            if let Some((val, _)) = self.entries.remove(&key) {
                self.current_bytes -= val.len() as u64;
            }
        }
    }

    /// 当前使用量
    pub fn usage(&self) -> u64 {
        self.current_bytes
    }

    /// 条目数
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 清空缓存
    pub fn clear(&mut self) {
        self.entries.clear();
        self.entry_order.clear();
        self.current_bytes = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_budget_default() {
        let budget = MemoryBudget::default();
        assert_eq!(budget.max_idle_bytes, 40 * 1024 * 1024);
        assert_eq!(budget.max_task_bytes, 120 * 1024 * 1024);
        assert_eq!(budget.cache_budget_bytes, 16 * 1024 * 1024);
        assert_eq!(budget.journal_index_budget_bytes, 8 * 1024 * 1024);
    }

    #[test]
    fn test_memory_budget_strict_40mb() {
        let budget = MemoryBudget::strict_40mb();
        assert_eq!(budget.max_idle_bytes, 40 * 1024 * 1024);
    }

    #[test]
    fn test_memory_snapshot_capture() {
        let snapshot = MemorySnapshot::capture();
        assert!(snapshot.rss_bytes > 0 || snapshot.vms_bytes > 0);
    }

    #[test]
    fn test_memory_snapshot_exceeds() {
        let budget = MemoryBudget::relaxed();
        let snapshot = MemorySnapshot {
            rss_bytes: 0,
            vms_bytes: 0,
            available_bytes: 0,
            timestamp: Instant::now(),
        };
        assert!(!snapshot.exceeds_idle(&budget));
        assert!(!snapshot.exceeds_task(&budget));

        let small_budget = MemoryBudget {
            max_idle_bytes: 1,
            max_task_bytes: 1,
            cache_budget_bytes: 1,
            journal_index_budget_bytes: 1,
        };
        let big_snapshot = MemorySnapshot {
            rss_bytes: 1024,
            vms_bytes: 1024,
            available_bytes: 0,
            timestamp: Instant::now(),
        };
        assert!(big_snapshot.exceeds_idle(&small_budget));
        assert!(big_snapshot.exceeds_task(&small_budget));
    }

    #[test]
    fn test_budget_enforcer_basic() {
        let enforcer = MemoryBudgetEnforcer::new(MemoryBudget::relaxed());
        assert_eq!(enforcer.cache_usage(), 0);
        assert_eq!(enforcer.journal_usage(), 0);
        assert_eq!(enforcer.over_budget_count(), 0);
    }

    #[test]
    fn test_budget_enforcer_usage_tracking() {
        let enforcer = MemoryBudgetEnforcer::new(MemoryBudget::relaxed());

        enforcer.add_cache_usage(1024);
        enforcer.add_journal_usage(512);
        assert_eq!(enforcer.cache_usage(), 1024);
        assert_eq!(enforcer.journal_usage(), 512);

        enforcer.release_cache_usage(256);
        enforcer.release_journal_usage(128);
        assert_eq!(enforcer.cache_usage(), 768);
        assert_eq!(enforcer.journal_usage(), 384);
    }

    #[test]
    fn test_budget_enforcer_force_shrink() {
        let enforcer = MemoryBudgetEnforcer::new(MemoryBudget::relaxed());
        enforcer.add_cache_usage(1024);
        enforcer.add_journal_usage(512);

        let actions = enforcer.force_shrink();
        assert!(actions.contains(&BudgetAction::GcRun));
        assert_eq!(enforcer.cache_usage(), 512);
        assert_eq!(enforcer.journal_usage(), 256);
    }

    #[test]
    fn test_cache_budget_insert_and_get() {
        let mut cache: CacheBudget<String> = CacheBudget::new(1024);

        assert!(cache.insert("key1".to_string(), vec![1, 2, 3]));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.usage(), 3);

        let val = cache.get(&"key1".to_string()).unwrap();
        assert_eq!(val, &vec![1, 2, 3]);
    }

    #[test]
    fn test_cache_budget_eviction() {
        let mut cache: CacheBudget<String> = CacheBudget::new(10);

        assert!(cache.insert("a".to_string(), vec![0; 4]));
        assert!(cache.insert("b".to_string(), vec![0; 4]));
        assert!(cache.insert("c".to_string(), vec![0; 4]));

        assert_eq!(cache.len(), 2);
        assert!(cache.usage() <= 10);

        assert!(cache.get(&"a".to_string()).is_none());
        assert!(cache.get(&"b".to_string()).is_some());
        assert!(cache.get(&"c".to_string()).is_some());
    }

    #[test]
    fn test_cache_budget_oversized_entry() {
        let mut cache: CacheBudget<String> = CacheBudget::new(4);
        assert!(!cache.insert("big".to_string(), vec![0; 8]));
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_cache_budget_clear() {
        let mut cache: CacheBudget<String> = CacheBudget::new(1024);
        cache.insert("a".to_string(), vec![1, 2, 3]);
        cache.insert("b".to_string(), vec![4, 5, 6]);
        assert_eq!(cache.len(), 2);

        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.usage(), 0);
    }

    #[test]
    fn test_cache_budget_lru_order() {
        let mut cache: CacheBudget<String> = CacheBudget::new(10);

        cache.insert("a".to_string(), vec![0; 3]);
        cache.insert("b".to_string(), vec![0; 3]);
        cache.insert("c".to_string(), vec![0; 3]);

        cache.get(&"a".to_string());

        cache.insert("d".to_string(), vec![0; 3]);

        assert!(cache.get(&"a".to_string()).is_some());
        assert!(cache.get(&"b".to_string()).is_none());
        assert!(cache.get(&"c".to_string()).is_some());
        assert!(cache.get(&"d".to_string()).is_some());
    }
}