use std::collections::BTreeMap;
use std::io::{Cursor, Read};

use cobblestone_core::{
    CHUNK_BLOCK_COUNT, CHUNK_COLUMN_COUNT, CHUNK_LIFECYCLE_MASK, CHUNK_NIBBLE_BYTES, ChunkCoord,
    ChunkImport, ChunkSnapshot, MAX_LEGACY_STATE_ID,
};

use crate::{
    CHUNK_RECORD_VERSION, MAX_CHUNK_PAYLOAD_BYTES, MAX_CHUNK_RECORD_BYTES,
    SEMANTIC_PAYLOAD_VERSION, StorageError,
};

const RECORD_MAGIC: &[u8; 4] = b"CBCH";
const RECORD_HEADER_BYTES: usize = 64;
const SEMANTIC_BASE_BYTES: usize =
    CHUNK_BLOCK_COUNT + CHUNK_NIBBLE_BYTES * 3 + CHUNK_COLUMN_COUNT * 2 + 4;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum Compression {
    None = 0,
    #[default]
    Zstd = 1,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub enum CompressionPolicy {
    None,
    Zstd,
    #[default]
    Adaptive,
}

pub const ADAPTIVE_COMPRESSION_MIN_SAVINGS: usize = 4096;

impl Compression {
    fn from_byte(value: u8) -> Result<Self, StorageError> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Zstd),
            other => Err(StorageError::UnsupportedCompression(other)),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ExtensionSection {
    pub tag: u16,
    pub version: u16,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct StoredChunk {
    pub position: ChunkCoord,
    pub compression: Compression,
    pub import: ChunkImport,
    pub extensions: Vec<ExtensionSection>,
}

pub fn encode_chunk_record(
    snapshot: &ChunkSnapshot,
    compression: Compression,
    extensions: &[ExtensionSection],
) -> Result<Vec<u8>, StorageError> {
    let policy = match compression {
        Compression::None => CompressionPolicy::None,
        Compression::Zstd => CompressionPolicy::Zstd,
    };
    encode_chunk_record_with_policy(snapshot, policy, extensions)
}

pub fn encode_chunk_record_with_policy(
    snapshot: &ChunkSnapshot,
    policy: CompressionPolicy,
    extensions: &[ExtensionSection],
) -> Result<Vec<u8>, StorageError> {
    let payload = encode_semantic_payload(snapshot, extensions)?;
    let payload_len = payload.len();
    let payload_crc = crc32c::crc32c(&payload);
    let (compression, stored) = match policy {
        CompressionPolicy::None => (Compression::None, payload),
        CompressionPolicy::Zstd => (
            Compression::Zstd,
            zstd::stream::encode_all(payload.as_slice(), 1)?,
        ),
        CompressionPolicy::Adaptive => {
            let compressed = zstd::stream::encode_all(payload.as_slice(), 1)?;
            if compressed
                .len()
                .saturating_add(ADAPTIVE_COMPRESSION_MIN_SAVINGS)
                <= payload_len
            {
                (Compression::Zstd, compressed)
            } else {
                (Compression::None, payload)
            }
        }
    };

    if stored.len() > MAX_CHUNK_PAYLOAD_BYTES {
        return Err(StorageError::PayloadTooLarge {
            size: stored.len(),
            limit: MAX_CHUNK_PAYLOAD_BYTES,
        });
    }

    let uncompressed_len =
        u32::try_from(payload_len).map_err(|_| StorageError::PayloadTooLarge {
            size: payload_len,
            limit: u32::MAX as usize,
        })?;
    let stored_len = u32::try_from(stored.len()).map_err(|_| StorageError::PayloadTooLarge {
        size: stored.len(),
        limit: u32::MAX as usize,
    })?;

    let mut header = [0_u8; RECORD_HEADER_BYTES];
    header[0..4].copy_from_slice(RECORD_MAGIC);
    put_u16(&mut header, 4, CHUNK_RECORD_VERSION);
    put_u16(&mut header, 6, RECORD_HEADER_BYTES as u16);
    put_i32(&mut header, 8, snapshot.position().x());
    put_i32(&mut header, 12, snapshot.position().z());
    put_u64(&mut header, 16, snapshot.terrain_revision());
    put_u64(&mut header, 24, snapshot.light_revision());
    header[32] = snapshot.lifecycle_flags();
    header[33] = compression as u8;
    put_u16(&mut header, 34, SEMANTIC_PAYLOAD_VERSION);
    put_u32(&mut header, 36, uncompressed_len);
    put_u32(&mut header, 40, stored_len);
    put_u32(&mut header, 44, payload_crc);
    let header_crc = crc32c::crc32c(&header[..60]);
    put_u32(&mut header, 60, header_crc);

    let record_len =
        RECORD_HEADER_BYTES
            .checked_add(stored.len())
            .ok_or(StorageError::RecordTooLarge {
                size: usize::MAX,
                limit: MAX_CHUNK_RECORD_BYTES,
            })?;
    if record_len > MAX_CHUNK_RECORD_BYTES {
        return Err(StorageError::RecordTooLarge {
            size: record_len,
            limit: MAX_CHUNK_RECORD_BYTES,
        });
    }

    let mut record = Vec::with_capacity(record_len);
    record.extend_from_slice(&header);
    record.extend_from_slice(&stored);
    Ok(record)
}

pub fn decode_chunk_record(record: &[u8]) -> Result<StoredChunk, StorageError> {
    if record.len() < RECORD_HEADER_BYTES {
        return Err(StorageError::CorruptChunkRecord("truncated header"));
    }
    if record.len() > MAX_CHUNK_RECORD_BYTES {
        return Err(StorageError::RecordTooLarge {
            size: record.len(),
            limit: MAX_CHUNK_RECORD_BYTES,
        });
    }

    let header = &record[..RECORD_HEADER_BYTES];
    if &header[0..4] != RECORD_MAGIC {
        return Err(StorageError::CorruptChunkRecord("bad magic"));
    }
    if read_u16(header, 4) != CHUNK_RECORD_VERSION {
        return Err(StorageError::CorruptChunkRecord(
            "unsupported record version",
        ));
    }
    if usize::from(read_u16(header, 6)) != RECORD_HEADER_BYTES {
        return Err(StorageError::CorruptChunkRecord("bad header length"));
    }
    if read_u16(header, 34) != SEMANTIC_PAYLOAD_VERSION {
        return Err(StorageError::CorruptChunkRecord(
            "unsupported semantic payload version",
        ));
    }
    if crc32c::crc32c(&header[..60]) != read_u32(header, 60) {
        return Err(StorageError::CorruptChunkRecord("header checksum mismatch"));
    }

    let lifecycle_flags = header[32];
    if lifecycle_flags & !CHUNK_LIFECYCLE_MASK != 0 {
        return Err(StorageError::CorruptChunkRecord("invalid lifecycle flags"));
    }
    let compression = Compression::from_byte(header[33])?;
    let uncompressed_len = read_u32(header, 36) as usize;
    let stored_len = read_u32(header, 40) as usize;
    if uncompressed_len > MAX_CHUNK_PAYLOAD_BYTES || stored_len > MAX_CHUNK_PAYLOAD_BYTES {
        return Err(StorageError::PayloadTooLarge {
            size: uncompressed_len.max(stored_len),
            limit: MAX_CHUNK_PAYLOAD_BYTES,
        });
    }
    if record.len() != RECORD_HEADER_BYTES + stored_len {
        return Err(StorageError::CorruptChunkRecord(
            "stored payload length mismatch",
        ));
    }

    let stored = &record[RECORD_HEADER_BYTES..];
    let payload = match compression {
        Compression::None => {
            if stored_len != uncompressed_len {
                return Err(StorageError::CorruptChunkRecord(
                    "uncompressed payload length mismatch",
                ));
            }
            stored.to_vec()
        }
        Compression::Zstd => {
            let decoder = zstd::stream::read::Decoder::new(Cursor::new(stored))?;
            let mut limited = decoder.take(uncompressed_len as u64 + 1);
            let mut decoded = Vec::with_capacity(uncompressed_len);
            limited.read_to_end(&mut decoded)?;
            if decoded.len() != uncompressed_len {
                return Err(StorageError::CorruptChunkRecord(
                    "zstd output length mismatch",
                ));
            }
            decoded
        }
    };

    if crc32c::crc32c(&payload) != read_u32(header, 44) {
        return Err(StorageError::CorruptChunkRecord(
            "payload checksum mismatch",
        ));
    }

    let position = ChunkCoord::new(read_i32(header, 8), read_i32(header, 12));
    let (states, sky_light, block_light, biomes, height_map, extra_data, extensions) =
        decode_semantic_payload(&payload)?;

    Ok(StoredChunk {
        position,
        compression,
        import: ChunkImport {
            terrain_revision: read_u64(header, 16),
            light_revision: read_u64(header, 24),
            lifecycle_flags,
            states,
            sky_light,
            block_light,
            biomes,
            height_map,
            extra_data,
        },
        extensions,
    })
}

fn encode_semantic_payload(
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
fn decode_semantic_payload(
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

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn put_i32(bytes: &mut [u8], offset: usize, value: i32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ])
}

fn read_i32(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}
