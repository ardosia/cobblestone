#![forbid(unsafe_code)]

mod buffer;
mod handle;
mod runtime;

pub use buffer::NativeBuffer;
pub use handle::{Arena, Handle, InsertError};
pub use runtime::RuntimeId;
