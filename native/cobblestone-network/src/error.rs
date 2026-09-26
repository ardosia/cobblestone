// Adapted for Cobblestone from Ardosia network transport code under Apache-2.0.
// See docs/provenance/ARDOSIA_REUSE.md for exact source revisions and modifications.

use std::io;

use thiserror::Error;

use crate::config::NetworkConfigError;

/// Errors produced by listener, connection, or backend operations.
#[derive(Debug, Error)]
pub enum NetworkError {
    /// The supplied network configuration could not be used.
    #[error(transparent)]
    Configuration(
        /// The typed configuration failure.
        #[from]
        NetworkConfigError,
    ),

    /// The underlying transport encountered an I/O failure.
    #[error("network I/O failed: {0}")]
    Io(
        /// The underlying I/O error.
        #[from]
        io::Error,
    ),

    /// The peer connection has already closed.
    #[error("connection is closed")]
    ConnectionClosed,

    /// Cobblestone closed the peer because its bounded inbound queue filled.
    #[error("connection closed by Cobblestone inbound backpressure policy")]
    Backpressure,

    /// The bounded backend command queue is currently full.
    #[error("network command queue is full")]
    CommandBackpressure,

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
