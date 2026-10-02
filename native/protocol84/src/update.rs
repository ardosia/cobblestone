use cobblestone_runtime::NativeBuffer;

use crate::{CodecError, RawPacket};

pub const UPDATE_BLOCK_ID: u8 = 0x13;
pub const UPDATE_BLOCK_FLAG_NEIGHBORS: u8 = 0x01;
pub const UPDATE_BLOCK_FLAG_NETWORK: u8 = 0x02;
pub const UPDATE_BLOCK_FLAG_NOGRAPHIC: u8 = 0x04;
pub const UPDATE_BLOCK_FLAG_PRIORITY: u8 = 0x08;
pub const UPDATE_BLOCK_FLAG_ALL_PRIORITY: u8 =
    UPDATE_BLOCK_FLAG_NEIGHBORS | UPDATE_BLOCK_FLAG_NETWORK | UPDATE_BLOCK_FLAG_PRIORITY;

/// Encodes one authoritative protocol-84 UpdateBlock packet.
///
/// The fixed-target body is big-endian x/z, one-byte y, one-byte block id, then flags in the
/// high nibble and legacy block data in the low nibble.
pub fn encode_protocol84_update_block(
    x: i32,
    y: u8,
    z: i32,
    state_id: u16,
    flags: u8,
) -> Result<RawPacket, CodecError> {
    if y > 127 {
        return Err(CodecError::InvalidFixedTargetValue {
            field: "UpdateBlock y",
            value: u64::from(y),
            max: 127,
        });
    }
    if state_id > 0x0fff {
        return Err(CodecError::InvalidFixedTargetValue {
            field: "UpdateBlock state id",
            value: u64::from(state_id),
            max: 0x0fff,
        });
    }
    if flags > 0x0f {
        return Err(CodecError::InvalidFixedTargetValue {
            field: "UpdateBlock flags",
            value: u64::from(flags),
            max: 0x0f,
        });
    }

    let mut body = Vec::with_capacity(11);
    body.extend_from_slice(&x.to_be_bytes());
    body.extend_from_slice(&z.to_be_bytes());
    body.push(y);
    body.push((state_id >> 4) as u8);
    body.push((flags << 4) | (state_id as u8 & 0x0f));

    Ok(RawPacket::new(
        UPDATE_BLOCK_ID,
        NativeBuffer::from_vec(body),
    ))
}

#[cfg(test)]
mod tests {
    use super::{UPDATE_BLOCK_FLAG_ALL_PRIORITY, UPDATE_BLOCK_ID, encode_protocol84_update_block};

    #[test]
    fn protocol84_update_block_matches_fixed_target_layout() {
        let packet = encode_protocol84_update_block(
            -2,
            127,
            0x0102_0304,
            0x05a,
            UPDATE_BLOCK_FLAG_ALL_PRIORITY,
        )
        .unwrap();

        assert_eq!(packet.id(), UPDATE_BLOCK_ID);
        assert_eq!(
            packet.body().as_slice(),
            &[
                0xff, 0xff, 0xff, 0xfe, 0x01, 0x02, 0x03, 0x04, 0x7f, 0x05, 0xba
            ],
        );
    }
}
