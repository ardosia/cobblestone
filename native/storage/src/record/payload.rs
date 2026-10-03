use std::collections::BTreeMap;

use cobblestone_world::{
    CHUNK_BLOCK_COUNT, CHUNK_COLUMN_COUNT, CHUNK_NIBBLE_BYTES, ChunkSnapshot, MAX_LEGACY_STATE_ID,
};

use super::ExtensionSection;
use crate::{MAX_CHUNK_PAYLOAD_BYTES, StorageError};

const SEMANTIC_BASE_BYTES: usize =
    CHUNK_BLOCK_COUNT + CHUNK_NIBBLE_BYTES * 3 + CHUNK_COLUMN_COUNT * 2 + 4;

pub(super) fn encode_semantic_payload(
    snapshot: &ChunkSnapshot,
    extensions: &[ExtensionSection],
) -> Result<Vec<u8>, StorageError> {
    if snapshot.states().len() != CHUNK_BLOCK_COUNT
        || snapshot.sky_light().len() != CHUNK_NIBBLE_BYTES
        || snapshot.block_light().len() != CHUNK_NIBBLE_BYTES
        || snapshot.biomes().len() != CHUNK_COLUMN_COUNT
        || snapshot.height_map().len() != CHUNK_COLUMN_COUNT
    {
        return Err(StorageError::InvalidSnapshot("plane length mismatch"));
    }

    let extension_bytes = extensions.iter().try_fold(0_usize, |total, section| {
        total
            .checked_add(8)
            .and_then(|value| value.checked_add(section.payload.len()))
            .ok_or(StorageError::PayloadTooLarge {
                size: usize::MAX,
                limit: MAX_CHUNK_PAYLOAD_BYTES,
            })
    })?;
    let extra_bytes =
        snapshot
            .extra_data()
            .len()
            .checked_mul(4)
            .ok_or(StorageError::PayloadTooLarge {
                size: usize::MAX,
                limit: MAX_CHUNK_PAYLOAD_BYTES,
            })?;
    let capacity = SEMANTIC_BASE_BYTES
        .checked_add(extra_bytes)
        .and_then(|value| value.checked_add(extension_bytes))
        .ok_or(StorageError::PayloadTooLarge {
            size: usize::MAX,
            limit: MAX_CHUNK_PAYLOAD_BYTES,
        })?;
    if capacity > MAX_CHUNK_PAYLOAD_BYTES {
        return Err(StorageError::PayloadTooLarge {
            size: capacity,
            limit: MAX_CHUNK_PAYLOAD_BYTES,
        });
    }

    let mut payload = Vec::with_capacity(capacity);
    let mut block_data = vec![0_u8; CHUNK_NIBBLE_BYTES];
    for (index, &state) in snapshot.states().iter().enumerate() {
        if state > MAX_LEGACY_STATE_ID {
            return Err(StorageError::InvalidSnapshot(
                "state exceeds fixed-target 12-bit range",
            ));
        }
        payload.push((state >> 4) as u8);
        let nibble = (state & 0x0f) as u8;
        let byte = &mut block_data[index >> 1];
        if index & 1 == 0 {
            *byte = (*byte & 0xf0) | nibble;
        } else {
            *byte = (*byte & 0x0f) | (nibble << 4);
        }
    }

    payload.extend_from_slice(&block_data);
    payload.extend_from_slice(snapshot.sky_light());
    payload.extend_from_slice(snapshot.block_light());
    payload.extend_from_slice(snapshot.biomes());
    payload.extend_from_slice(snapshot.height_map());

    let extra_count = u32::try_from(snapshot.extra_data().len())
        .map_err(|_| StorageError::InvalidSnapshot("extra-data count exceeds u32"))?;
    payload.extend_from_slice(&extra_count.to_le_bytes());
    for (&key, &value) in snapshot.extra_data() {
        payload.extend_from_slice(&extra_key_to_linear(key)?.to_le_bytes());
        payload.extend_from_slice(&value.to_le_bytes());
    }

    for section in extensions {
        let len =
            u32::try_from(section.payload.len()).map_err(|_| StorageError::PayloadTooLarge {
                size: section.payload.len(),
                limit: u32::MAX as usize,
            })?;
        payload.extend_from_slice(&section.tag.to_le_bytes());
        payload.extend_from_slice(&section.version.to_le_bytes());
        payload.extend_from_slice(&len.to_le_bytes());
        payload.extend_from_slice(&section.payload);
    }
    Ok(payload)
}

#[allow(clippy::type_complexity)]
pub(super) fn decode_semantic_payload(
    payload: &[u8],
) -> Result<
    (
        Vec<u16>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        BTreeMap<u16, u16>,
        Vec<ExtensionSection>,
    ),
    StorageError,
> {
    if payload.len() < SEMANTIC_BASE_BYTES {
        return Err(StorageError::CorruptChunkPayload("truncated base planes"));
    }

    let ids_end = CHUNK_BLOCK_COUNT;
    let data_end = ids_end + CHUNK_NIBBLE_BYTES;
    let sky_end = data_end + CHUNK_NIBBLE_BYTES;
    let block_end = sky_end + CHUNK_NIBBLE_BYTES;
    let biome_end = block_end + CHUNK_COLUMN_COUNT;
    let height_end = biome_end + CHUNK_COLUMN_COUNT;

    let ids = &payload[..ids_end];
    let data = &payload[ids_end..data_end];
    let mut states = Vec::with_capacity(CHUNK_BLOCK_COUNT);
    for index in 0..CHUNK_BLOCK_COUNT {
        let nibble = if index & 1 == 0 {
            data[index >> 1] & 0x0f
        } else {
            data[index >> 1] >> 4
        };
        states.push((u16::from(ids[index]) << 4) | u16::from(nibble));
    }

    let sky_light = payload[data_end..sky_end].to_vec();
    let block_light = payload[sky_end..block_end].to_vec();
    let biomes = payload[block_end..biome_end].to_vec();
    let height_map = payload[biome_end..height_end].to_vec();

    let mut cursor = height_end;
    let extra_count = read_payload_u32(payload, &mut cursor)? as usize;
    if extra_count > CHUNK_BLOCK_COUNT {
        return Err(StorageError::CorruptChunkPayload(
            "extra-data count exceeds block count",
        ));
    }

    let extra_bytes = extra_count
        .checked_mul(4)
        .ok_or(StorageError::CorruptChunkPayload(
            "extra-data size overflow",
        ))?;
    if payload.len().saturating_sub(cursor) < extra_bytes {
        return Err(StorageError::CorruptChunkPayload(
            "truncated extra-data entries",
        ));
    }

    let mut extra_data = BTreeMap::new();
    for _ in 0..extra_count {
        let linear = read_payload_u16(payload, &mut cursor)?;
        let value = read_payload_u16(payload, &mut cursor)?;
        if usize::from(linear) >= CHUNK_BLOCK_COUNT {
            return Err(StorageError::CorruptChunkPayload(
                "extra-data block index out of range",
            ));
        }
        let key = linear_to_extra_key(linear);
        if extra_data.insert(key, value).is_some() {
            return Err(StorageError::CorruptChunkPayload(
                "duplicate extra-data block index",
            ));
        }
    }

    let mut extensions = Vec::new();
    while cursor < payload.len() {
        if payload.len() - cursor < 8 {
            return Err(StorageError::CorruptChunkPayload(
                "truncated extension header",
            ));
        }
        let tag = read_payload_u16(payload, &mut cursor)?;
        let version = read_payload_u16(payload, &mut cursor)?;
        let len = read_payload_u32(payload, &mut cursor)? as usize;
        let end = cursor
            .checked_add(len)
            .ok_or(StorageError::CorruptChunkPayload(
                "extension length overflow",
            ))?;
        if end > payload.len() {
            return Err(StorageError::CorruptChunkPayload(
                "truncated extension payload",
            ));
        }
        extensions.push(ExtensionSection {
            tag,
            version,
            payload: payload[cursor..end].to_vec(),
        });
        cursor = end;
    }

    Ok((
        states,
        sky_light,
        block_light,
        biomes,
        height_map,
        extra_data,
        extensions,
    ))
}

fn extra_key_to_linear(key: u16) -> Result<u16, StorageError> {
    let z = (key >> 12) & 0x0f;
    let x = (key >> 8) & 0x0f;
    let y = key & 0x00ff;
    if y >= 128 {
        return Err(StorageError::InvalidSnapshot(
            "extra-data internal Y is out of range",
        ));
    }
    Ok((y << 8) | (z << 4) | x)
}

fn linear_to_extra_key(index: u16) -> u16 {
    let x = index & 0x0f;
    let z = (index >> 4) & 0x0f;
    let y = (index >> 8) & 0x7f;
    (z << 12) | (x << 8) | y
}

fn read_payload_u16(payload: &[u8], cursor: &mut usize) -> Result<u16, StorageError> {
    let end = cursor
        .checked_add(2)
        .ok_or(StorageError::CorruptChunkPayload("payload offset overflow"))?;
    let bytes = payload
        .get(*cursor..end)
        .ok_or(StorageError::CorruptChunkPayload("truncated u16"))?;
    *cursor = end;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_payload_u32(payload: &[u8], cursor: &mut usize) -> Result<u32, StorageError> {
    let end = cursor
        .checked_add(4)
        .ok_or(StorageError::CorruptChunkPayload("payload offset overflow"))?;
    let bytes = payload
        .get(*cursor..end)
        .ok_or(StorageError::CorruptChunkPayload("truncated u32"))?;
    *cursor = end;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}
