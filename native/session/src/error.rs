use cobblestone_raknet::RaknetError;
use cobblestone_wire::CodecError;
use thiserror::Error;

/// Failures surfaced by the production session mechanism.
#[derive(Debug, Error)]
pub enum SessionError {
    /// The underlying bounded RakNet transport rejected or lost the operation.
    #[error(transparent)]
    Raknet(#[from] RaknetError),

    /// An inbound connected payload was not valid within the configured fixed-target limits.
    ///
    /// The session closes the peer before returning this error.
    #[error("malformed session input: {source}")]
    MalformedInput {
        /// Codec failure that made the inbound payload invalid.
        #[source]
        source: CodecError,
    },

    /// An outbound packet could not be encoded within the configured fixed-target limits.
    #[error("invalid outbound session packet: {source}")]
    OutboundCodec {
        /// Codec failure that prevented transmission.
        #[source]
        source: CodecError,
    },

    /// The process-local nonzero session identity space was exhausted.
    #[error("session identity space exhausted")]
    IdExhausted,
}
