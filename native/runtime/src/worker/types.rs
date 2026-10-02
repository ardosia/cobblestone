use core::fmt;
use core::num::NonZeroU64;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Stable identity for one accepted native worker task.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct TaskId(pub(super) NonZeroU64);

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
    pub(super) fn new() -> Self {
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
    pub(super) id: TaskId,
    pub(super) cancellation: CancellationToken,
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
    pub(super) completions: Vec<Completion<R>>,
    pub(super) worker_panics: usize,
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
