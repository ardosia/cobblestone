use cobblestone_core::NativeBuffer;

use crate::{CodecError, RawPacket};

/// Fixed-target FullChunkData packet ID.
pub const FULL_CHUNK_DATA_ID: u8 = 0x34;
/// Fixed-target layered chunk order.
pub const CHUNK_ORDER_LAYERED: u8 = 1;
/// Number of block IDs in one 16x16x128 chunk.
pub const CHUNK_BLOCK_COUNT: usize = 16 * 16 * 128;
/// Number of bytes in one packed-nibble chunk plane.
pub const CHUNK_NIBBLE_BYTES: usize = CHUNK_BLOCK_COUNT / 2;
/// Number of horizontal columns in one chunk.
pub const CHUNK_COLUMN_COUNT: usize = 16 * 16;

/// Borrowed semantic chunk state ready for protocol-84 projection.
///
/// Block and nibble planes use Cobblestone's semantic Y/Z/X ordering. Biomes are semantic IDs.
/// This type deliberately is not the protocol wire layout.
#[derive(Debug, Clone, Copy)]
pub struct Protocol84ChunkSnapshot<'a> {
    /// Chunk X coordinate.
    pub chunk_x: i32,
    /// Chunk Z coordinate.
    pub chunk_z: i32,
    /// Semantic block-ID plane.
    pub block_ids: &'a [u8],
    /// Semantic block metadata/data nibble plane.
    pub block_data: &'a [u8],
    /// Semantic sky-light nibble plane.
    pub sky_light: &'a [u8],
    /// Semantic block-light nibble plane.
    pub block_light: &'a [u8],
    /// Per-column semantic biome IDs in Z/X order.
    pub biomes: &'a [u8],
    /// Per-column height map in Z/X order.
    pub height_map: &'a [u8],
    /// Sparse fixed-target block-extra-data entries keyed by (z << 12) | (x << 8) | y.
    pub extra_data: &'a [(u32, u16)],
}

/// Encodes one semantic chunk snapshot as protocol-84 FullChunkData using layered order.
///
/// Protocol-84 ORDER_LAYERED consumes the same Y/Z/X block and nibble plane order used by the
/// semantic snapshot. Height map stays byte-per-column, biome IDs become fixed-target biome
/// ID/color words, and sparse extra data is little-endian.
pub fn encode_protocol84_full_chunk_data(
    snapshot: Protocol84ChunkSnapshot<'_>,
) -> Result<RawPacket, CodecError> {
    require_len("chunk block ids", snapshot.block_ids, CHUNK_BLOCK_COUNT)?;
    require_len("chunk block data", snapshot.block_data, CHUNK_NIBBLE_BYTES)?;
    require_len("chunk sky light", snapshot.sky_light, CHUNK_NIBBLE_BYTES)?;
    require_len(
        "chunk block light",
        snapshot.block_light,
        CHUNK_NIBBLE_BYTES,
    )?;
    require_len("chunk biomes", snapshot.biomes, CHUNK_COLUMN_COUNT)?;
    require_len("chunk height map", snapshot.height_map, CHUNK_COLUMN_COUNT)?;

    let extra_bytes =
        snapshot
            .extra_data
            .len()
            .checked_mul(6)
            .ok_or(CodecError::LengthOutOfRange {
                field: "chunk extra data",
                value: snapshot.extra_data.len(),
                max: usize::MAX / 6,
            })?;
    let mut payload = Vec::with_capacity(
        CHUNK_BLOCK_COUNT
            + CHUNK_NIBBLE_BYTES * 3
            + CHUNK_COLUMN_COUNT
            + CHUNK_COLUMN_COUNT * 4
            + 4
            + extra_bytes,
    );
    payload.extend_from_slice(snapshot.block_ids);
    payload.extend_from_slice(snapshot.block_data);
    payload.extend_from_slice(snapshot.sky_light);
    payload.extend_from_slice(snapshot.block_light);
    payload.extend_from_slice(snapshot.height_map);

    for &biome in snapshot.biomes {
        payload.extend_from_slice(&fixed_target_biome_word(biome)?.to_be_bytes());
    }

    let extra_count =
        u32::try_from(snapshot.extra_data.len()).map_err(|_| CodecError::LengthOutOfRange {
            field: "chunk extra-data count",
            value: snapshot.extra_data.len(),
            max: u32::MAX as usize,
        })?;
    payload.extend_from_slice(&extra_count.to_le_bytes());
    for &(key, value) in snapshot.extra_data {
        validate_extra_data_key(key)?;
        payload.extend_from_slice(&key.to_le_bytes());
        payload.extend_from_slice(&value.to_le_bytes());
    }

    let payload_len = u32::try_from(payload.len()).map_err(|_| CodecError::LengthOutOfRange {
        field: "full chunk payload",
        value: payload.len(),
        max: u32::MAX as usize,
    })?;

    let mut body = Vec::with_capacity(13 + payload.len());
    body.extend_from_slice(&snapshot.chunk_x.to_be_bytes());
    body.extend_from_slice(&snapshot.chunk_z.to_be_bytes());
    body.push(CHUNK_ORDER_LAYERED);
    body.extend_from_slice(&payload_len.to_be_bytes());
    body.extend_from_slice(&payload);

    Ok(RawPacket::new(
        FULL_CHUNK_DATA_ID,
        NativeBuffer::from_vec(body),
    ))
}

fn require_len(field: &'static str, bytes: &[u8], expected: usize) -> Result<(), CodecError> {
    if bytes.len() == expected {
        Ok(())
    } else {
        Err(CodecError::InvalidChunkPlaneLength {
            field,
            expected,
            actual: bytes.len(),
        })
    }
}

fn fixed_target_biome_word(id: u8) -> Result<u32, CodecError> {
    match id {
        // 0.15.10 Plains: biome ID 1 in the high byte, historical grass RGB 0x92bc59.
        1 => Ok(0x0192_bc59),
        _ => Err(CodecError::UnsupportedChunkBiome { id }),
    }
}

fn validate_extra_data_key(key: u32) -> Result<(), CodecError> {
    let y = key & 0xff;
    if key > 0xffff || y > 127 {
        return Err(CodecError::InvalidChunkExtraDataKey { key });
    }
    Ok(())
}

