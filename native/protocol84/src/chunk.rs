use cobblestone_runtime::NativeBuffer;
use cobblestone_target::ChunkShape;

use crate::{CodecError, NbtDocument, NbtLimits, RawPacket};

/// Fixed-target FullChunkData packet ID.
pub const FULL_CHUNK_DATA_ID: u8 = 0x34;
/// Fixed-target layered chunk order.
pub const CHUNK_ORDER_LAYERED: u8 = 1;
/// Number of block IDs in one 16x16x128 chunk.
pub const CHUNK_BLOCK_COUNT: usize = ChunkShape::BLOCK_COUNT;
/// Number of bytes in one packed-nibble chunk plane.
pub const CHUNK_NIBBLE_BYTES: usize = ChunkShape::NIBBLE_BYTES;
/// Number of horizontal columns in one chunk.
pub const CHUNK_COLUMN_COUNT: usize = ChunkShape::COLUMN_COUNT;

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
    /// Per-column fixed-target biome words: high byte ID, low 24 bits RGB.
    pub biome_words: &'a [u32],
    /// Per-column height map in Z/X order.
    pub height_map: &'a [u8],
    /// Sparse fixed-target block-extra-data entries keyed by (z << 12) | (x << 8) | y.
    pub extra_data: &'a [(u32, u16)],
    /// Sequential little-endian NBT block-entity documents appended after extra data.
    pub block_entities: &'a [NbtDocument],
}

/// Encodes the protocol-84 client-side chunk-unload action.
pub fn encode_protocol84_chunk_unload(_chunk_x: i32, _chunk_z: i32) -> Option<RawPacket> {
    // MCPE 0.15.10 / protocol 84 has no dedicated chunk-unload packet.
    // The client evicts terrain outside its negotiated view distance as the
    // player moves; the historical server only forgets the chunk locally
    // (and separately despawns entities) instead of sending a terrain packet.
    None
}

/// Encodes one semantic chunk snapshot as protocol-84 FullChunkData using layered order.
///
/// Protocol-84 ORDER_LAYERED consumes the same Y/Z/X block and nibble plane order used by the
/// semantic snapshot. Height map stays byte-per-column, stored biome words are emitted
/// big-endian unchanged, and sparse extra data is little-endian.
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
    if snapshot.biome_words.len() != CHUNK_COLUMN_COUNT {
        return Err(CodecError::InvalidChunkPlaneLength {
            field: "chunk biome words",
            expected: CHUNK_COLUMN_COUNT,
            actual: snapshot.biome_words.len(),
        });
    }
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
    let nbt_limits = NbtLimits::new(64 * 1024, 16, 256, 1024);
    let encoded_block_entities = snapshot
        .block_entities
        .iter()
        .map(|document| {
            document
                .encode_le(nbt_limits)
                .map(|buffer| buffer.as_slice().to_vec())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let block_entity_bytes = encoded_block_entities
        .iter()
        .try_fold(0_usize, |total, bytes| {
            total
                .checked_add(bytes.len())
                .ok_or(CodecError::LengthOutOfRange {
                    field: "chunk block entities",
                    value: usize::MAX,
                    max: u32::MAX as usize,
                })
        })?;

    let mut payload = Vec::with_capacity(
        CHUNK_BLOCK_COUNT
            + CHUNK_NIBBLE_BYTES * 3
            + CHUNK_COLUMN_COUNT
            + CHUNK_COLUMN_COUNT * 4
            + 4
            + extra_bytes
            + block_entity_bytes,
    );
    payload.extend_from_slice(snapshot.block_ids);
    payload.extend_from_slice(snapshot.block_data);
    payload.extend_from_slice(snapshot.sky_light);
    payload.extend_from_slice(snapshot.block_light);
    payload.extend_from_slice(snapshot.height_map);

    for &word in snapshot.biome_words {
        payload.extend_from_slice(&word.to_be_bytes());
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
    for block_entity in encoded_block_entities {
        payload.extend_from_slice(&block_entity);
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

fn validate_extra_data_key(key: u32) -> Result<(), CodecError> {
    let y = key & 0xff;
    if key > 0xffff || y > 127 {
        return Err(CodecError::InvalidChunkExtraDataKey { key });
    }
    Ok(())
}
