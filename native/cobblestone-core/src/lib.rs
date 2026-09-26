#![forbid(unsafe_code)]

mod buffer;
mod handle;
mod ownership;
mod runtime;
mod worker;

pub use buffer::NativeBuffer;
pub use handle::{Arena, Handle, InsertError};
pub use ownership::{
    OwnedArena, OwnedHandle, OwnershipEpoch, OwnershipError, OwnershipMetadata,
};
pub use runtime::RuntimeId;
pub use worker::{
    CancellationToken, Completion, ShutdownReport, TaskHandle, TaskId, TrySubmitError, WorkerPool,
    WorkerPoolBuildError,
};
