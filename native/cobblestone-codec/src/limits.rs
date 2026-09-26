/// Historical Login decoder cap observed in the matching protocol-84 source.
pub const LOGIN_MAX_DECOMPRESSED_BYTES: usize = 2 * 1024 * 1024;

/// Caller-supplied operational bounds for untrusted protocol-84 bytes.
///
/// These are Cobblestone resource limits, not wire-protocol constants. Callers choose them for the
/// deployment/session policy instead of the codec inventing modern-Bedrock defaults.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct CodecLimits {
    max_frame_bytes: usize,
    max_login_compressed_bytes: usize,
    max_batch_compressed_bytes: usize,
    max_batch_decompressed_bytes: usize,
    max_inner_packet_bytes: usize,
    max_inner_packets: usize,
}

impl CodecLimits {
    /// Creates an explicit set of operational bounds.
    pub const fn new(
        max_frame_bytes: usize,
        max_login_compressed_bytes: usize,
        max_batch_compressed_bytes: usize,
        max_batch_decompressed_bytes: usize,
        max_inner_packet_bytes: usize,
        max_inner_packets: usize,
    ) -> Self {
        Self {
            max_frame_bytes,
            max_login_compressed_bytes,
            max_batch_compressed_bytes,
            max_batch_decompressed_bytes,
            max_inner_packet_bytes,
            max_inner_packets,
        }
    }

    /// Maximum complete game-frame bytes.
    pub const fn max_frame_bytes(self) -> usize {
        self.max_frame_bytes
    }

    /// Maximum compressed Login payload bytes.
    pub const fn max_login_compressed_bytes(self) -> usize {
        self.max_login_compressed_bytes
    }

    /// Maximum compressed Batch payload bytes.
    pub const fn max_batch_compressed_bytes(self) -> usize {
        self.max_batch_compressed_bytes
    }

    /// Maximum decompressed Batch bytes.
    pub const fn max_batch_decompressed_bytes(self) -> usize {
        self.max_batch_decompressed_bytes
    }

    /// Maximum bytes in one raw packet nested in a Batch.
    pub const fn max_inner_packet_bytes(self) -> usize {
        self.max_inner_packet_bytes
    }

    /// Maximum packet count in one Batch.
    pub const fn max_inner_packets(self) -> usize {
        self.max_inner_packets
    }
}
