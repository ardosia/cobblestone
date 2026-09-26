use std::io;

use thiserror::Error;

use crate::config::NetworkConfigError;

/// Errors produced by listener, connection, or backend operations.
#[derive(Debug, Error)]
pub enum NetworkError {
    /// The supplied network configuration could not be used.
    #[error(transparent)]
    Configuration(#[from] NetworkConfigError),

    /// The underlying transport encountered an I/O failure.
    #[error("network I/O failed: {0}")]
    Io(#[from] io::Error),

    /// The peer connection has already closed.
    #[error("connection is closed")]
    ConnectionClosed,

    /// Cobblestone closed the peer because a bounded queue reached its backpressure limit.
    #[error("connection closed by Cobblestone backpressure policy")]
    Backpressure,

    /// The asynchronous network backend is no longer available.
    #[error("network backend stopped")]
    BackendStopped,

    /// A transport worker or backend task failed unexpectedly.
    #[error("network backend failed: {message}")]
    BackendFailure {
        /// Human-readable backend failure context.
        message: String,
    },
}
