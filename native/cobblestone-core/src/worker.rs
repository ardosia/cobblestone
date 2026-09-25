use core::fmt;
use core::num::NonZeroU64;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{
    Receiver, RecvError, SyncSender, TryRecvError, TrySendError, sync_channel,
};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

/// Stable identity for one accepted native worker task.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct TaskId(NonZeroU64);

impl TaskId {
    /// Returns the nonzero integer representation.
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Cooperative cancellation state shared by a task handle and the worker.
///
/// Cancellation prevents execution when observed before the task starts. Once a task is running,
/// its handler must check this token at appropriate cancellation points.
#[derive(Clone, Debug)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Requests cancellation.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// Whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

/// Handle returned for an accepted worker task.
#[derive(Clone, Debug)]
pub struct TaskHandle {
    id: TaskId,
    cancellation: CancellationToken,
}

impl TaskHandle {
    /// Stable task identity.
    pub const fn id(&self) -> TaskId {
        self.id
    }

    /// Requests cancellation.
    pub fn cancel(&self) {
        self.cancellation.cancel();
    }

    /// Whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }
}

/// Completion emitted by the bounded native worker pool.
#[derive(Debug)]
pub enum Completion<R> {
    /// The handler returned normally.
    Completed { id: TaskId, result: R },
    /// Cancellation was observed before the handler started.
    Cancelled { id: TaskId },
    /// The handler panicked. The panic was contained inside the worker thread boundary.
    Panicked { id: TaskId },
}

impl<R> Completion<R> {
    /// Task identity associated with this completion.
    pub const fn id(&self) -> TaskId {
        match self {
            Self::Completed { id, .. } | Self::Cancelled { id } | Self::Panicked { id } => *id,
        }
    }
}

/// Why a nonblocking task submission was rejected.
#[derive(Debug)]
pub enum TrySubmitError<J> {
    /// The bounded job queue is full. The original job is returned to the caller.
    Full(J),
    /// The worker pool is shutting down. The original job is returned to the caller.
    Shutdown(J),
    /// Task identity space has been exhausted. The original job is returned to the caller.
    IdExhausted(J),
}

impl<J> TrySubmitError<J> {
    /// Recovers the job that was not accepted.
    pub fn into_job(self) -> J {
        match self {
            Self::Full(job) | Self::Shutdown(job) | Self::IdExhausted(job) => job,
        }
    }
}

/// Worker-pool construction failure.
#[derive(Debug)]
pub enum WorkerPoolBuildError {
    /// At least one worker thread is required.
    ZeroWorkers,
    /// The submission queue must have nonzero capacity.
    ZeroJobCapacity,
    /// The completion queue must have nonzero capacity.
    ZeroCompletionCapacity,
    /// The operating system could not start a worker thread.
    Spawn(std::io::Error),
}

impl fmt::Display for WorkerPoolBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroWorkers => f.write_str("worker count must be nonzero"),
            Self::ZeroJobCapacity => f.write_str("job queue capacity must be nonzero"),
            Self::ZeroCompletionCapacity => {
                f.write_str("completion queue capacity must be nonzero")
            }
            Self::Spawn(error) => write!(f, "failed to spawn worker thread: {error}"),
        }
    }
}

impl std::error::Error for WorkerPoolBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn(error) => Some(error),
            Self::ZeroWorkers | Self::ZeroJobCapacity | Self::ZeroCompletionCapacity => None,
        }
    }
}

/// Result of graceful worker-pool shutdown.
///
/// Shutdown stops accepting new jobs, drains every already-accepted job, drains completions so
/// workers cannot deadlock on a full completion queue, and then joins every worker thread.
#[derive(Debug)]
pub struct ShutdownReport<R> {
    completions: Vec<Completion<R>>,
    worker_panics: usize,
}

impl<R> ShutdownReport<R> {
    /// Completions drained while shutting down.
    pub fn completions(&self) -> &[Completion<R>] {
        &self.completions
    }

    /// Consumes the report and returns the drained completions.
    pub fn into_completions(self) -> Vec<Completion<R>> {
        self.completions
    }

    /// Number of worker threads that terminated with an unexpected internal panic.
    ///
    /// Handler panics are contained and reported as `Completion::Panicked`, so they do not
    /// increment this count.
    pub const fn worker_panics(&self) -> usize {
        self.worker_panics
    }
}

struct QueuedJob<J> {
    id: TaskId,
    cancellation: CancellationToken,
    job: J,
}

/// Bounded native worker pool.
///
/// Submission is nonblocking and reports saturation explicitly. The completion queue is also
/// bounded; workers block when the owner does not drain completions, naturally propagating
/// backpressure until submission also saturates.
///
/// The worker handler receives only owned task input plus a cancellation token. This crate has no
/// Zend/PHP dependency, so worker threads cannot invoke arbitrary PHP APIs through this surface.
pub struct WorkerPool<J: Send + 'static, R: Send + 'static> {
    jobs: Option<SyncSender<QueuedJob<J>>>,
    completions: Receiver<Completion<R>>,
    workers: Vec<JoinHandle<()>>,
    next_task_id: AtomicU64,
}

impl<J: Send + 'static, R: Send + 'static> WorkerPool<J, R> {
    /// Creates a bounded worker pool.
    pub fn new<F>(
        worker_count: usize,
        job_capacity: usize,
        completion_capacity: usize,
        handler: F,
    ) -> Result<Self, WorkerPoolBuildError>
    where
        F: Fn(J, CancellationToken) -> R + Send + Sync + 'static,
    {
        if worker_count == 0 {
            return Err(WorkerPoolBuildError::ZeroWorkers);
        }
        if job_capacity == 0 {
            return Err(WorkerPoolBuildError::ZeroJobCapacity);
        }
        if completion_capacity == 0 {
            return Err(WorkerPoolBuildError::ZeroCompletionCapacity);
        }

        let (job_tx, job_rx) = sync_channel(job_capacity);
        let (completion_tx, completion_rx) = sync_channel(completion_capacity);
        let job_rx = Arc::new(Mutex::new(job_rx));
        let handler: Arc<dyn Fn(J, CancellationToken) -> R + Send + Sync> = Arc::new(handler);
        let mut workers = Vec::with_capacity(worker_count);

        for index in 0..worker_count {
            let worker_job_rx = Arc::clone(&job_rx);
            let worker_completion_tx = completion_tx.clone();
            let worker_handler = Arc::clone(&handler);
            let spawn = thread::Builder::new()
                .name(format!("cobblestone-worker-{index}"))
                .spawn(move || {
                    worker_loop(worker_job_rx, worker_completion_tx, worker_handler);
                });

            match spawn {
                Ok(worker) => workers.push(worker),
                Err(error) => {
                    drop(job_tx);
                    drop(completion_tx);
                    for worker in workers {
                        let _ = worker.join();
                    }
                    return Err(WorkerPoolBuildError::Spawn(error));
                }
            }
        }

        // Only workers own completion senders. This lets shutdown detect that all workers exited
        // when the completion receiver becomes disconnected.
        drop(completion_tx);

        Ok(Self {
            jobs: Some(job_tx),
            completions: completion_rx,
            workers,
            next_task_id: AtomicU64::new(1),
        })
    }

    /// Attempts to submit a job without blocking.
    ///
    /// Queue saturation is returned as `TrySubmitError::Full`; the pool never grows an
    /// unbounded submission queue.
    pub fn try_submit(&self, job: J) -> Result<TaskHandle, TrySubmitError<J>> {
        let id = match self.allocate_task_id() {
            Some(id) => id,
            None => return Err(TrySubmitError::IdExhausted(job)),
        };
        let cancellation = CancellationToken::new();
        let queued = QueuedJob {
            id,
            cancellation: cancellation.clone(),
            job,
        };

        let Some(sender) = self.jobs.as_ref() else {
            return Err(TrySubmitError::Shutdown(queued.job));
        };

        match sender.try_send(queued) {
            Ok(()) => Ok(TaskHandle { id, cancellation }),
            Err(TrySendError::Full(queued)) => Err(TrySubmitError::Full(queued.job)),
            Err(TrySendError::Disconnected(queued)) => {
                Err(TrySubmitError::Shutdown(queued.job))
            }
        }
    }

    /// Attempts to receive one completion without blocking.
    pub fn try_recv_completion(&self) -> Result<Completion<R>, TryRecvError> {
        self.completions.try_recv()
    }

    /// Waits for one completion.
    pub fn recv_completion(&self) -> Result<Completion<R>, RecvError> {
        self.completions.recv()
    }

    /// Stops accepting work, drains accepted jobs/completions, and joins all workers.
    pub fn shutdown(mut self) -> ShutdownReport<R> {
        self.shutdown_inner()
    }

    fn allocate_task_id(&self) -> Option<TaskId> {
        let current = self
            .next_task_id
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .ok()?;
        NonZeroU64::new(current).map(TaskId)
    }

    fn shutdown_inner(&mut self) -> ShutdownReport<R> {
        // Dropping the sole job sender rejects future work and lets workers exit after draining
        // every job already accepted by the bounded queue.
        self.jobs.take();

        // Drain until every worker has dropped its completion sender. Draining here is essential:
        // a worker may otherwise be blocked by the bounded completion queue during shutdown.
        let mut completions = Vec::new();
        while let Ok(completion) = self.completions.recv() {
            completions.push(completion);
        }

        let mut worker_panics = 0;
        for worker in self.workers.drain(..) {
            if worker.join().is_err() {
                worker_panics += 1;
            }
        }

        ShutdownReport {
            completions,
            worker_panics,
        }
    }
}

impl<J: Send + 'static, R: Send + 'static> Drop for WorkerPool<J, R> {
    fn drop(&mut self) {
        let _ = self.shutdown_inner();
    }
}

fn worker_loop<J: Send + 'static, R: Send + 'static>(
    jobs: Arc<Mutex<Receiver<QueuedJob<J>>>>,
    completions: SyncSender<Completion<R>>,
    handler: Arc<dyn Fn(J, CancellationToken) -> R + Send + Sync>,
) {
    loop {
        let queued = {
            let receiver = match jobs.lock() {
                Ok(receiver) => receiver,
                Err(poisoned) => poisoned.into_inner(),
            };
            match receiver.recv() {
                Ok(queued) => queued,
                Err(_) => break,
            }
        };

        let completion = if queued.cancellation.is_cancelled() {
            Completion::Cancelled { id: queued.id }
        } else {
            let id = queued.id;
            match catch_unwind(AssertUnwindSafe(|| {
                (handler)(queued.job, queued.cancellation)
            })) {
                Ok(result) => Completion::Completed { id, result },
                Err(_) => Completion::Panicked { id },
            }
        };

        if completions.send(completion).is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::thread;

    use super::{Completion, TrySubmitError, WorkerPool};

    fn wait_until(flag: &AtomicBool) {
        while !flag.load(Ordering::Acquire) {
            thread::yield_now();
        }
    }

    #[test]
    fn full_job_queue_reports_backpressure_and_returns_job() {
        let started = Arc::new(AtomicBool::new(false));
        let release = Arc::new(AtomicBool::new(false));
        let worker_started = Arc::clone(&started);
        let worker_release = Arc::clone(&release);

        let pool = WorkerPool::new(1, 1, 2, move |job: u32, _| {
            worker_started.store(true, Ordering::Release);
            while !worker_release.load(Ordering::Acquire) {
                thread::yield_now();
            }
            job * 2
        })
        .expect("worker pool");

        pool.try_submit(1).expect("first job accepted");
        wait_until(&started);
        pool.try_submit(2).expect("second job queued");

        match pool.try_submit(3) {
            Err(TrySubmitError::Full(job)) => assert_eq!(job, 3),
            other => panic!("expected full queue, got {other:?}"),
        }

        release.store(true, Ordering::Release);
        let report = pool.shutdown();
        assert_eq!(report.completions().len(), 2);
        assert_eq!(report.worker_panics(), 0);
    }

    #[test]
    fn cancellation_before_start_skips_handler() {
        let started = Arc::new(AtomicBool::new(false));
        let release = Arc::new(AtomicBool::new(false));
        let calls = Arc::new(AtomicUsize::new(0));
        let worker_started = Arc::clone(&started);
        let worker_release = Arc::clone(&release);
        let worker_calls = Arc::clone(&calls);

        let pool = WorkerPool::new(1, 2, 2, move |job: u32, _| {
            worker_calls.fetch_add(1, Ordering::Relaxed);
            if job == 1 {
                worker_started.store(true, Ordering::Release);
                while !worker_release.load(Ordering::Acquire) {
                    thread::yield_now();
                }
            }
            job
        })
        .expect("worker pool");

        pool.try_submit(1).expect("first job accepted");
        wait_until(&started);
        let cancelled = pool.try_submit(2).expect("second job accepted");
        cancelled.cancel();
        assert!(cancelled.is_cancelled());

        release.store(true, Ordering::Release);
        let completions = pool.shutdown().into_completions();
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert!(completions.iter().any(
            |completion| matches!(completion, Completion::Cancelled { id } if *id == cancelled.id())
        ));
    }

    #[test]
    fn shutdown_drains_accepted_work_even_when_completion_queue_is_small() {
        let pool = WorkerPool::new(2, 4, 1, |job: u32, _| job + 10).expect("worker pool");

        for job in 0..4 {
            pool.try_submit(job).expect("job accepted");
        }

        let report = pool.shutdown();
        assert_eq!(report.completions().len(), 4);
        assert_eq!(report.worker_panics(), 0);

        let mut results = report
            .into_completions()
            .into_iter()
            .filter_map(|completion| match completion {
                Completion::Completed { result, .. } => Some(result),
                Completion::Cancelled { .. } | Completion::Panicked { .. } => None,
            })
            .collect::<Vec<_>>();
        results.sort_unstable();
        assert_eq!(results, vec![10, 11, 12, 13]);
    }

    #[test]
    fn handler_panic_is_contained_and_worker_continues() {
        let pool = WorkerPool::new(1, 2, 2, |job: u32, _| {
            assert_ne!(job, 0, "intentional test panic");
            job * 2
        })
        .expect("worker pool");

        let panicking = pool.try_submit(0).expect("panic job accepted");
        let healthy = pool.try_submit(5).expect("healthy job accepted");
        let completions = pool.shutdown().into_completions();

        assert!(completions.iter().any(
            |completion| matches!(completion, Completion::Panicked { id } if *id == panicking.id())
        ));
        assert!(completions.iter().any(
            |completion| matches!(
                completion,
                Completion::Completed { id, result: 10 } if *id == healthy.id()
            )
        ));
    }
}
