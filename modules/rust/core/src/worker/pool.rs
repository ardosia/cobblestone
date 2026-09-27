use core::num::NonZeroU64;
use crossbeam_channel::{
    Receiver, Sender, TryRecvError as CrossbeamTryRecvError, TrySendError, bounded,
};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{RecvError, TryRecvError};
use std::thread::{self, JoinHandle};

use super::types::{
    CancellationToken, Completion, ShutdownReport, TaskHandle, TaskId, TrySubmitError,
    WorkerPoolBuildError,
};

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
    jobs: Option<Sender<QueuedJob<J>>>,
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

        let (job_tx, job_rx) = bounded(job_capacity);
        let (completion_tx, completion_rx) = bounded(completion_capacity);
        let handler: Arc<dyn Fn(J, CancellationToken) -> R + Send + Sync> = Arc::new(handler);
        let mut workers = Vec::with_capacity(worker_count);

        for index in 0..worker_count {
            let worker_job_rx = job_rx.clone();
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
                    drop(job_rx);
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
            Err(TrySendError::Disconnected(queued)) => Err(TrySubmitError::Shutdown(queued.job)),
        }
    }

    /// Attempts to receive one completion without blocking.
    pub fn try_recv_completion(&self) -> Result<Completion<R>, TryRecvError> {
        self.completions.try_recv().map_err(|error| match error {
            CrossbeamTryRecvError::Empty => TryRecvError::Empty,
            CrossbeamTryRecvError::Disconnected => TryRecvError::Disconnected,
        })
    }

    /// Waits for one completion.
    pub fn recv_completion(&self) -> Result<Completion<R>, RecvError> {
        self.completions.recv().map_err(|_| RecvError)
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
    jobs: Receiver<QueuedJob<J>>,
    completions: Sender<Completion<R>>,
    handler: Arc<dyn Fn(J, CancellationToken) -> R + Send + Sync>,
) {
    while let Ok(queued) = jobs.recv() {
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
        assert!(completions.iter().any(|completion| matches!(
            completion,
            Completion::Completed { id, result: 10 } if *id == healthy.id()
        )));
    }
}
