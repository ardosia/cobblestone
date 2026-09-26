use thiserror::Error;

/// Resource category associated with a configured codec limit.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum LimitKind {
    /// Complete RakNet-connected game frame bytes.
    Frame,
    /// Compressed bytes carried by a Login packet.
    LoginCompressed,
    /// Compressed bytes carried by a Batch packet.
    BatchCompressed,
    /// Decompressed bytes produced by a Batch packet.
    BatchDecompressed,
    /// One raw packet nested inside a Batch packet.
    InnerPacket,
    /// Number of packets nested inside a Batch packet.
    InnerPacketCount,
    /// Evidence-backed maximum decompressed Login payload.
    LoginDecompressed,
}

/// Typed failures returned for malformed or resource-invalid protocol-84 input.
#[derive(Debug, Error, Clone, Eq, PartialEq)]
pub enum CodecError {
    /// Input ended before the requested primitive could be read.
    #[error("truncated input: needed {needed} bytes with only {remaining} remaining")]
    UnexpectedEof {
        /// Bytes required by the current read.
        needed: usize,
        /// Bytes still available.
        remaining: usize,
    },

    /// The outer connected payload did not begin with the MCPE game marker.
    #[error("invalid game marker 0x{actual:02x}")]
    InvalidGameMarker {
        /// Observed first byte.
        actual: u8,
    },

    /// A signed wire length was negative.
    #[error("negative length for {field}: {value}")]
    NegativeLength {
        /// Length field name.
        field: &'static str,
        /// Observed signed length.
        value: i32,
    },

    /// A host-side length could not fit into the target wire field.
    #[error("length for {field} exceeds wire maximum: {value} > {max}")]
    LengthOutOfRange {
        /// Length field name.
        field: &'static str,
        /// Actual byte length.
        value: usize,
        /// Largest encodable value.
        max: usize,
    },

    /// Input exceeded an explicit codec resource limit.
    #[error("{kind:?} limit exceeded: {actual} > {limit}")]
    LimitExceeded {
        /// Resource category.
        kind: LimitKind,
        /// Configured maximum.
        limit: usize,
        /// Observed amount.
        actual: usize,
    },

    /// A length-delimited value did not consume the containing input exactly.
    #[error("trailing bytes after decoding: {remaining}")]
    TrailingBytes {
        /// Unconsumed bytes.
        remaining: usize,
    },

    /// A raw packet had no packet ID byte.
    #[error("raw packet is empty")]
    EmptyPacket,

    /// A typed bootstrap decoder does not implement the observed packet ID.
    #[error("unsupported bootstrap packet id 0x{id:02x}")]
    UnsupportedPacket {
        /// Observed packet ID.
        id: u8,
    },

    /// A Login packet did not target protocol 84.
    #[error("unsupported game protocol {actual}; expected {expected}")]
    UnsupportedProtocol {
        /// Fixed target protocol.
        expected: i32,
        /// Observed protocol.
        actual: i32,
    },

    /// A text field was not valid UTF-8.
    #[error("invalid UTF-8 string")]
    InvalidUtf8,

    /// Zlib compression or decompression failed.
    #[error("zlib failure: {message}")]
    Compression {
        /// Stable error text from the compressor/decompressor.
        message: String,
    },
}
