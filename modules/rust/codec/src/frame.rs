use cobblestone_core::NativeBuffer;

use crate::{CodecError, CodecLimits, LimitKind};

/// Protocol-84 connected game-data marker carried inside RakNet payloads.
pub const GAME_MARKER: u8 = 0xfe;

/// One raw protocol-84 packet: packet ID plus opaque body bytes.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RawPacket {
    id: u8,
    body: NativeBuffer,
}

impl RawPacket {
    /// Creates a raw packet from an ID and immutable body.
    pub fn new(id: u8, body: NativeBuffer) -> Self {
        Self { id, body }
    }

    /// Packet ID byte.
    pub const fn id(&self) -> u8 {
        self.id
    }

    /// Opaque bytes after the packet ID.
    pub const fn body(&self) -> &NativeBuffer {
        &self.body
    }

    pub(crate) fn from_packet_bytes(bytes: &[u8]) -> Result<Self, CodecError> {
        let Some((&id, body)) = bytes.split_first() else {
            return Err(CodecError::EmptyPacket);
        };
        Ok(Self::new(id, NativeBuffer::copy_from_slice(body)))
    }

    pub(crate) fn encoded_len(&self) -> usize {
        1 + self.body.len()
    }

    pub(crate) fn write_packet_bytes(&self, output: &mut Vec<u8>) {
        output.push(self.id);
        output.extend_from_slice(self.body.as_slice());
    }
}

/// Decodes the outer 0xfe game marker and packet ID from one connected payload.
pub fn decode_game_frame(input: &[u8], limits: CodecLimits) -> Result<RawPacket, CodecError> {
    check_limit(LimitKind::Frame, input.len(), limits.max_frame_bytes())?;
    let Some((&marker, packet)) = input.split_first() else {
        return Err(CodecError::UnexpectedEof {
            needed: 1,
            remaining: 0,
        });
    };
    if marker != GAME_MARKER {
        return Err(CodecError::InvalidGameMarker { actual: marker });
    }
    RawPacket::from_packet_bytes(packet)
}

/// Encodes one raw protocol-84 packet as a 0xfe connected game frame.
pub fn encode_game_frame(
    packet: &RawPacket,
    limits: CodecLimits,
) -> Result<NativeBuffer, CodecError> {
    let len = 1_usize
        .checked_add(packet.encoded_len())
        .ok_or(CodecError::LengthOutOfRange {
            field: "game frame",
            value: usize::MAX,
            max: limits.max_frame_bytes(),
        })?;
    check_limit(LimitKind::Frame, len, limits.max_frame_bytes())?;

    let mut output = Vec::with_capacity(len);
    output.push(GAME_MARKER);
    packet.write_packet_bytes(&mut output);
    Ok(NativeBuffer::from_vec(output))
}

pub(crate) fn check_limit(kind: LimitKind, actual: usize, limit: usize) -> Result<(), CodecError> {
    if actual > limit {
        Err(CodecError::LimitExceeded {
            kind,
            limit,
            actual,
        })
    } else {
        Ok(())
    }
}
