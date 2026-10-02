use std::io;

/// Synchronous failures exposed by the native session host.
#[derive(Debug, thiserror::Error)]
pub enum SessionHostError {
    /// The dedicated native host thread could not be started.
    #[error("failed to spawn session host thread: {0}")]
    ThreadSpawn(#[source] io::Error),
    /// Tokio could not create the dedicated host runtime.
    #[error("failed to build session host runtime: {0}")]
    RuntimeBuild(#[source] io::Error),
    /// The listener/session mechanism failed before the host became ready.
    #[error("session host startup failed: {message}")]
    Startup {
        /// Failure text from the native session mechanism.
        message: String,
    },
    /// Startup ended without reporting success or failure.
    #[error("session host startup channel closed")]
    StartupClosed,
    /// No live session currently owns this process-local identity.
    #[error("unknown or stale session id {session_id}")]
    UnknownSession {
        /// Requested process-local identity.
        session_id: u64,
    },
    /// The bounded command queue for this session is full.
    #[error("session {session_id} command queue is full")]
    CommandBackpressure {
        /// Requested process-local identity.
        session_id: u64,
    },
    /// The session task has already stopped accepting commands.
    #[error("session {session_id} command queue is closed")]
    CommandClosed {
        /// Requested process-local identity.
        session_id: u64,
    },
    /// The owner-side event receiver has disconnected.
    #[error("session host event channel is closed")]
    EventChannelClosed,
    /// The host shutdown signal could not be delivered.
    #[error("session host shutdown channel is closed")]
    ShutdownChannelClosed,
    /// The dedicated session host thread panicked.
    #[error("session host thread panicked")]
    ThreadPanicked,
}
