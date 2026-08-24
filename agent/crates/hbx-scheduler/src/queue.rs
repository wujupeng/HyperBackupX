use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use hbx_core::domain::common::JobId;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskPriority {
    Low = 0,
    #[default]
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl PartialOrd for TaskPriority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TaskPriority {
    fn cmp(&self, other: &Self) -> Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskKind {
    Backup,
    Restore,
    Verify,
    Cleanup,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedTask {
    pub task_id: Uuid,
    pub job_id: JobId,
    pub priority: TaskPriority,
    pub kind: TaskKind,
    pub enqueued_at: DateTime<Utc>,
}

impl PartialEq for QueuedTask {
    fn eq(&self, other: &Self) -> bool {
        self.task_id == other.task_id
    }
}

impl Eq for QueuedTask {}

impl PartialOrd for QueuedTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QueuedTask {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.priority.cmp(&other.priority) {
            Ordering::Equal => other.enqueued_at.cmp(&self.enqueued_at),
            non_eq => non_eq,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DequeueError {
    QueueEmpty,
    JobAlreadyRunning,
    NoConcurrencySlot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnqueueError {
    JobAlreadyQueued,
}

struct QueueInner {
    pending: BinaryHeap<QueuedTask>,
    queued_jobs: HashSet<JobId>,
    running_tasks: HashSet<Uuid>,
    running_jobs: HashSet<JobId>,
}

#[derive(Clone)]
pub struct TaskQueue {
    inner: Arc<Mutex<QueueInner>>,
    max_concurrent: Arc<AtomicUsize>,
}

impl TaskQueue {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(QueueInner {
                pending: BinaryHeap::new(),
                queued_jobs: HashSet::new(),
                running_tasks: HashSet::new(),
                running_jobs: HashSet::new(),
            })),
            max_concurrent: Arc::new(AtomicUsize::new(max_concurrent)),
        }
    }

    pub fn max_concurrent(&self) -> usize {
        self.max_concurrent.load(AtomicOrdering::Relaxed)
    }

    pub fn set_max_concurrent(&self, max: usize) {
        self.max_concurrent.store(max.max(1), AtomicOrdering::Relaxed);
    }

    pub fn enqueue(&self, task: QueuedTask) -> Result<(), EnqueueError> {
        let mut inner = self.inner.lock();
        if inner.queued_jobs.contains(&task.job_id) || inner.running_jobs.contains(&task.job_id) {
            return Err(EnqueueError::JobAlreadyQueued);
        }
        inner.queued_jobs.insert(task.job_id.clone());
        inner.pending.push(task);
        Ok(())
    }

    pub fn dequeue(&self) -> Result<QueuedTask, DequeueError> {
        let mut inner = self.inner.lock();
        if inner.pending.is_empty() {
            return Err(DequeueError::QueueEmpty);
        }
        if inner.running_tasks.len() >= self.max_concurrent.load(AtomicOrdering::Relaxed) {
            return Err(DequeueError::NoConcurrencySlot);
        }

        let mut skipped: Vec<QueuedTask> = Vec::new();
        let mut found: Option<QueuedTask> = None;

        while let Some(task) = inner.pending.pop() {
            if inner.running_jobs.contains(&task.job_id) {
                skipped.push(task);
            } else {
                inner.queued_jobs.remove(&task.job_id);
                inner.running_tasks.insert(task.task_id);
                inner.running_jobs.insert(task.job_id.clone());
                found = Some(task);
                break;
            }
        }

        for t in skipped {
            inner.pending.push(t);
        }

        found.ok_or(DequeueError::QueueEmpty)
    }

    pub fn complete(&self, task_id: Uuid, job_id: &JobId) {
        let mut inner = self.inner.lock();
        inner.running_tasks.remove(&task_id);
        inner.running_jobs.remove(job_id);
    }

    pub fn cancel(&self, task_id: Uuid) -> Option<QueuedTask> {
        let mut inner = self.inner.lock();
        let mut found = None;
        let mut remaining = BinaryHeap::new();
        while let Some(task) = inner.pending.pop() {
            if task.task_id == task_id {
                found = Some(task);
            } else {
                remaining.push(task);
            }
        }
        inner.pending = remaining;
        if let Some(ref t) = found {
            inner.queued_jobs.remove(&t.job_id);
        }
        found
    }

    pub fn pending_count(&self) -> usize {
        self.inner.lock().pending.len()
    }

    pub fn running_count(&self) -> usize {
        self.inner.lock().running_tasks.len()
    }

    pub fn is_job_running(&self, job_id: &JobId) -> bool {
        self.inner.lock().running_jobs.contains(job_id)
    }

    pub fn is_job_queued(&self, job_id: &JobId) -> bool {
        let inner = self.inner.lock();
        inner.queued_jobs.contains(job_id) || inner.running_jobs.contains(job_id)
    }
}

impl Default for TaskQueue {
    fn default() -> Self {
        Self::new(2)
    }
}

pub fn make_task(job_id: JobId, priority: TaskPriority, kind: TaskKind) -> QueuedTask {
    QueuedTask {
        task_id: Uuid::new_v4(),
        job_id,
        priority,
        kind,
        enqueued_at: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbx_core::domain::common::JobId;
    use uuid::Uuid;

    fn job_id() -> JobId {
        JobId(Uuid::new_v4())
    }

    #[test]
    fn test_enqueue_dequeue_basic() {
        let queue = TaskQueue::new(2);
        let jid = job_id();
        let task = make_task(jid.clone(), TaskPriority::Normal, TaskKind::Backup);
        let tid = task.task_id;

        assert!(queue.enqueue(task).is_ok());
        assert_eq!(queue.pending_count(), 1);

        let dequeued = queue.dequeue().unwrap();
        assert_eq!(dequeued.task_id, tid);
        assert_eq!(queue.running_count(), 1);
        assert_eq!(queue.pending_count(), 0);

        queue.complete(tid, &jid);
        assert_eq!(queue.running_count(), 0);
    }

    #[test]
    fn test_priority_ordering() {
        let queue = TaskQueue::new(4);
        let j1 = job_id();
        let j2 = job_id();
        let j3 = job_id();

        let t_low = make_task(j1.clone(), TaskPriority::Low, TaskKind::Backup);
        let t_high = make_task(j2.clone(), TaskPriority::High, TaskKind::Backup);
        let t_normal = make_task(j3.clone(), TaskPriority::Normal, TaskKind::Backup);

        let id_low = t_low.task_id;
        let id_high = t_high.task_id;
        let id_normal = t_normal.task_id;

        queue.enqueue(t_low).unwrap();
        queue.enqueue(t_high).unwrap();
        queue.enqueue(t_normal).unwrap();

        let first = queue.dequeue().unwrap();
        assert_eq!(first.task_id, id_high);
        queue.complete(first.task_id, &first.job_id);

        let second = queue.dequeue().unwrap();
        assert_eq!(second.task_id, id_normal);
        queue.complete(second.task_id, &second.job_id);

        let third = queue.dequeue().unwrap();
        assert_eq!(third.task_id, id_low);
        queue.complete(third.task_id, &third.job_id);
    }

    #[test]
    fn test_critical_priority_preempts() {
        let queue = TaskQueue::new(1);
        let j1 = job_id();
        let j2 = job_id();

        let t_normal = make_task(j1.clone(), TaskPriority::Normal, TaskKind::Backup);
        let t_critical = make_task(j2.clone(), TaskPriority::Critical, TaskKind::Restore);


        let id_critical = t_critical.task_id;

        queue.enqueue(t_normal).unwrap();
        queue.enqueue(t_critical).unwrap();

        let first = queue.dequeue().unwrap();
        assert_eq!(first.task_id, id_critical);

        let second = queue.dequeue();
        assert!(second.is_err());
    }

    #[test]
    fn test_concurrency_limit() {
        let queue = TaskQueue::new(2);
        let j1 = job_id();
        let j2 = job_id();
        let j3 = job_id();

        let t1 = make_task(j1.clone(), TaskPriority::Normal, TaskKind::Backup);
        let t2 = make_task(j2.clone(), TaskPriority::Normal, TaskKind::Backup);
        let t3 = make_task(j3.clone(), TaskPriority::Normal, TaskKind::Backup);
        let id3 = t3.task_id;

        queue.enqueue(t1).unwrap();
        queue.enqueue(t2).unwrap();
        queue.enqueue(t3).unwrap();

        let d1 = queue.dequeue().unwrap();
        let _d2 = queue.dequeue().unwrap();
        let d3 = queue.dequeue();
        assert!(d3.is_err());
        assert_eq!(queue.running_count(), 2);

        queue.complete(d1.task_id, &d1.job_id);
        let d3 = queue.dequeue().unwrap();
        assert_eq!(d3.task_id, id3);
    }

    #[test]
    fn test_task_lock_same_job() {
        let queue = TaskQueue::new(4);
        let jid = job_id();

        let t1 = make_task(jid.clone(), TaskPriority::Normal, TaskKind::Backup);
        let t1_id = t1.task_id;
        queue.enqueue(t1).unwrap();

        let t2 = make_task(jid.clone(), TaskPriority::High, TaskKind::Backup);
        assert!(queue.enqueue(t2).is_err());

        let d1 = queue.dequeue().unwrap();
        assert_eq!(d1.task_id, t1_id);

        let t3 = make_task(jid.clone(), TaskPriority::High, TaskKind::Backup);
        assert!(queue.enqueue(t3).is_err());

        queue.complete(d1.task_id, &jid);

        let t4 = make_task(jid.clone(), TaskPriority::High, TaskKind::Backup);
        assert!(queue.enqueue(t4).is_ok());
    }

    #[test]
    fn test_hot_update_max_concurrent() {
        let queue = TaskQueue::new(1);
        assert_eq!(queue.max_concurrent(), 1);

        queue.set_max_concurrent(4);
        assert_eq!(queue.max_concurrent(), 4);

        let j1 = job_id();
        let j2 = job_id();
        let t1 = make_task(j1.clone(), TaskPriority::Normal, TaskKind::Backup);
        let t2 = make_task(j2.clone(), TaskPriority::Normal, TaskKind::Backup);

        queue.enqueue(t1).unwrap();
        queue.enqueue(t2).unwrap();

        let _d1 = queue.dequeue().unwrap();
        let _d2 = queue.dequeue().unwrap();
        assert_eq!(queue.running_count(), 2);
    }

    #[test]
    fn test_cancel_pending() {
        let queue = TaskQueue::new(2);
        let jid = job_id();
        let task = make_task(jid.clone(), TaskPriority::Normal, TaskKind::Backup);
        let tid = task.task_id;

        queue.enqueue(task).unwrap();
        assert_eq!(queue.pending_count(), 1);

        let cancelled = queue.cancel(tid).unwrap();
        assert_eq!(cancelled.task_id, tid);
        assert_eq!(queue.pending_count(), 0);
        assert!(!queue.is_job_queued(&jid));
    }

    #[test]
    fn test_dequeue_empty_queue() {
        let queue = TaskQueue::new(2);
        assert_eq!(queue.dequeue(), Err(DequeueError::QueueEmpty));
    }

    #[test]
    fn test_fifo_within_same_priority() {
        let queue = TaskQueue::new(3);
        let j1 = job_id();
        let j2 = job_id();

        let t1 = make_task(j1.clone(), TaskPriority::Normal, TaskKind::Backup);
        std::thread::sleep(std::time::Duration::from_millis(10));
        let t2 = make_task(j2.clone(), TaskPriority::Normal, TaskKind::Backup);

        let id1 = t1.task_id;
        let id2 = t2.task_id;

        queue.enqueue(t1).unwrap();
        queue.enqueue(t2).unwrap();

        let first = queue.dequeue().unwrap();
        assert_eq!(first.task_id, id1);
        queue.complete(first.task_id, &first.job_id);

        let second = queue.dequeue().unwrap();
        assert_eq!(second.task_id, id2);
    }

    #[test]
    fn test_is_job_running_and_queued() {
        let queue = TaskQueue::new(2);
        let jid = job_id();
        let task = make_task(jid.clone(), TaskPriority::Normal, TaskKind::Backup);

        queue.enqueue(task).unwrap();
        assert!(queue.is_job_queued(&jid));
        assert!(!queue.is_job_running(&jid));

        let d = queue.dequeue().unwrap();
        assert!(queue.is_job_running(&jid));
        assert!(queue.is_job_queued(&jid));

        queue.complete(d.task_id, &jid);
        assert!(!queue.is_job_running(&jid));
        assert!(!queue.is_job_queued(&jid));
    }

    #[test]
    fn test_default_max_concurrent_is_2() {
        let queue = TaskQueue::default();
        assert_eq!(queue.max_concurrent(), 2);
    }
}