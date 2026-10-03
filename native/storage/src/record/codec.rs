use std::io::{Cursor, Read};

use cobblestone_world::{CHUNK_LIFECYCLE_MASK, ChunkCoord, ChunkImport, ChunkSnapshot};

use super::{
    ADAPTIVE_COMPRESSION_MIN_SAVINGS, Compression, CompressionPolicy, ExtensionSection,
    StoredChunk,
    payload::{decode_semantic_payload, encode_semantic_payload},
};
use crate::{
    CHUNK_RECORD_VERSION, MAX_CHUNK_PAYLOAD_BYTES, MAX_CHUNK_RECORD_BYTES,
    SEMANTIC_PAYLOAD_VERSION, StorageError,
};

const RECORD_MAGIC: &[u8; 4] = b"CBCH";
const RECORD_HEADER_BYTES: usize = 64;

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
