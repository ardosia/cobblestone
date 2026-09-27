use crate::{SessionId, SessionPacket};

/// Event delivered from the native session mechanism to its owning runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionHostEvent {
    /// A new fixed-target gameplay session was accepted.
    Connected {
        /// Stable process-local session identity.
        session_id: SessionId,
        /// Remote socket address rendered without exposing transport types.
        peer: String,
    },
    /// One packet arrived after frame/Batch processing.
    Packet {
        /// Stable process-local session identity.
        session_id: SessionId,
        /// Decoded wire packet.
        packet: SessionPacket,
    },
    /// A session ended or was closed by a bounded policy.
    Disconnected {
        /// Stable process-local session identity.
        session_id: SessionId,
        /// Stable human-readable mechanism reason.
        reason: String,
    },
}
