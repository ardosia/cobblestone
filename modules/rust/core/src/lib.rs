#![forbid(unsafe_code)]

mod arena;
mod buffer;
mod handle;
mod ownership;
mod region;
mod runtime;
mod worker;

pub use arena::{Arena, InsertError};
pub use buffer::NativeBuffer;
pub use handle::Handle;
pub use ownership::{OwnedArena, OwnedHandle, OwnershipEpoch, OwnershipError, OwnershipMetadata};
pub use region::{RegionDirectory, RegionId, RegionRoute, RegionRouteError};
pub use runtime::RuntimeId;
pub use worker::{
    CancellationToken, Completion, ShutdownReport, TaskHandle, TaskId, TrySubmitError, WorkerPool,
    WorkerPoolBuildError,
};

