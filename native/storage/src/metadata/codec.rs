use super::{
    MAX_WORLD_METADATA_PAYLOAD_BYTES, TARGET_GAME_PROTOCOL, TARGET_RAKNET_PROTOCOL,
    TARGET_VERSION_MAJOR, TARGET_VERSION_MINOR, TARGET_VERSION_PATCH,
    WORLD_METADATA_ENVELOPE_BYTES, WORLD_METADATA_PAYLOAD_VERSION, WorldMetadata,
    validate_metadata,
};
use crate::{STORAGE_FORMAT_VERSION, StorageError};

const WORLD_METADATA_MAGIC: &[u8; 4] = b"CBWM";
const PAYLOAD_FIXED_BYTES: usize = 76;

pub fn encode_world_metadata(metadata: &WorldMetadata) -> Result<Vec<u8>, StorageError> {
    encode_world_metadata_inner(metadata)
}

pub fn decode_world_metadata(encoded: &[u8]) -> Result<WorldMetadata, StorageError> {
    decode_world_metadata_inner(encoded)
}

pub(super) fn encode_world_metadata_inner(
    metadata: &WorldMetadata,
) -> Result<Vec<u8>, StorageError> {
    validate_metadata(metadata)?;

    let name = metadata.name.as_bytes();
    let settings = metadata.generator_settings.as_slice();
    let payload_len = PAYLOAD_FIXED_BYTES
        .checked_add(name.len())
        .and_then(|size| size.checked_add(settings.len()))
        .ok_or(StorageError::WorldMetadataTooLarge {
            size: usize::MAX,
            limit: MAX_WORLD_METADATA_PAYLOAD_BYTES,
        })?;
    if payload_len > MAX_WORLD_METADATA_PAYLOAD_BYTES {
        return Err(StorageError::WorldMetadataTooLarge {
            size: payload_len,
            limit: MAX_WORLD_METADATA_PAYLOAD_BYTES,
        });
    }

    let mut payload = vec![0_u8; payload_len];
    put_u16(&mut payload, 0, WORLD_METADATA_PAYLOAD_VERSION);
    payload[2] = TARGET_VERSION_MAJOR;
    payload[3] = TARGET_VERSION_MINOR;
    payload[4] = TARGET_VERSION_PATCH;
    payload[5] = u8::from(metadata.time_running);
    put_u32(&mut payload, 8, TARGET_GAME_PROTOCOL);
    put_u32(&mut payload, 12, TARGET_RAKNET_PROTOCOL);
    payload[16..32].copy_from_slice(&metadata.world_uuid);
    put_i64(&mut payload, 32, metadata.seed);
    put_u32(&mut payload, 40, metadata.generator_id);
    put_u16(&mut payload, 44, metadata.generator_settings_version);
    put_i32(&mut payload, 48, metadata.spawn_x);
    put_i32(&mut payload, 52, metadata.spawn_y);
    put_i32(&mut payload, 56, metadata.spawn_z);
    put_i64(&mut payload, 60, metadata.time);
    put_u32(
        &mut payload,
        68,
        u32::try_from(name.len()).map_err(|_| StorageError::WorldMetadataTooLarge {
            size: name.len(),
            limit: u32::MAX as usize,
        })?,
    );
    put_u32(
        &mut payload,
        72,
        u32::try_from(settings.len()).map_err(|_| StorageError::WorldMetadataTooLarge {
            size: settings.len(),
            limit: u32::MAX as usize,
        })?,
    );
    payload[PAYLOAD_FIXED_BYTES..PAYLOAD_FIXED_BYTES + name.len()].copy_from_slice(name);
    payload[PAYLOAD_FIXED_BYTES + name.len()..].copy_from_slice(settings);

    let mut envelope = [0_u8; WORLD_METADATA_ENVELOPE_BYTES];
    envelope[0..4].copy_from_slice(WORLD_METADATA_MAGIC);
    put_u16(&mut envelope, 4, STORAGE_FORMAT_VERSION);
    put_u16(&mut envelope, 6, WORLD_METADATA_ENVELOPE_BYTES as u16);
    put_u64(&mut envelope, 8, metadata.generation);
    put_u32(
        &mut envelope,
        16,
        u32::try_from(payload.len()).map_err(|_| StorageError::WorldMetadataTooLarge {
            size: payload.len(),
            limit: u32::MAX as usize,
        })?,
    );
    put_u32(&mut envelope, 20, crc32c::crc32c(&payload));
    let envelope_crc = crc32c::crc32c(&envelope[..24]);
    put_u32(&mut envelope, 24, envelope_crc);

    let mut encoded = Vec::with_capacity(WORLD_METADATA_ENVELOPE_BYTES + payload.len());
    encoded.extend_from_slice(&envelope);
    encoded.extend_from_slice(&payload);
    Ok(encoded)
}

pub(super) fn decode_world_metadata_inner(encoded: &[u8]) -> Result<WorldMetadata, StorageError> {
    if encoded.len() < WORLD_METADATA_ENVELOPE_BYTES {
        return Err(StorageError::InvalidWorldMetadata("truncated envelope"));
    }
    let envelope = &encoded[..WORLD_METADATA_ENVELOPE_BYTES];
    if &envelope[0..4] != WORLD_METADATA_MAGIC {
        return Err(StorageError::InvalidWorldMetadata("bad magic"));
    }
    if read_u16(envelope, 4) != STORAGE_FORMAT_VERSION {
        return Err(StorageError::InvalidWorldMetadata(
            "unsupported storage format version",
        ));
    }
    if usize::from(read_u16(envelope, 6)) != WORLD_METADATA_ENVELOPE_BYTES {
        return Err(StorageError::InvalidWorldMetadata(
            "bad metadata envelope length",
        ));
    }
    if crc32c::crc32c(&envelope[..24]) != read_u32(envelope, 24) {
        return Err(StorageError::InvalidWorldMetadata(
            "metadata envelope checksum mismatch",
        ));
    }
    if envelope[28..32] != [0; 4] {
        return Err(StorageError::InvalidWorldMetadata(
            "metadata envelope reserved bytes are nonzero",
        ));
    }

    let payload_len = read_u32(envelope, 16) as usize;
    if payload_len > MAX_WORLD_METADATA_PAYLOAD_BYTES {
        return Err(StorageError::WorldMetadataTooLarge {
            size: payload_len,
            limit: MAX_WORLD_METADATA_PAYLOAD_BYTES,
        });
    }
    if encoded.len() != WORLD_METADATA_ENVELOPE_BYTES + payload_len {
        return Err(StorageError::InvalidWorldMetadata(
            "metadata payload length mismatch",
        ));
    }

    let payload = &encoded[WORLD_METADATA_ENVELOPE_BYTES..];
    if crc32c::crc32c(payload) != read_u32(envelope, 20) {
        return Err(StorageError::InvalidWorldMetadata(
            "metadata payload checksum mismatch",
        ));
    }
    if payload.len() < PAYLOAD_FIXED_BYTES {
        return Err(StorageError::InvalidWorldMetadata(
            "truncated semantic payload",
        ));
    }
    if read_u16(payload, 0) != WORLD_METADATA_PAYLOAD_VERSION {
        return Err(StorageError::InvalidWorldMetadata(
            "unsupported metadata payload version",
        ));
    }
    if payload[2] != TARGET_VERSION_MAJOR
        || payload[3] != TARGET_VERSION_MINOR
        || payload[4] != TARGET_VERSION_PATCH
        || read_u32(payload, 8) != TARGET_GAME_PROTOCOL
        || read_u32(payload, 12) != TARGET_RAKNET_PROTOCOL
    {
        return Err(StorageError::InvalidWorldMetadata(
            "fixed-target marker mismatch",
        ));
    }
    if payload[5] > 1 {
        return Err(StorageError::InvalidWorldMetadata(
            "invalid time-running flag",
        ));
    }
    if payload[6..8] != [0; 2] || payload[46..48] != [0; 2] {
        return Err(StorageError::InvalidWorldMetadata(
            "metadata payload reserved bytes are nonzero",
        ));
    }

    let name_len = read_u32(payload, 68) as usize;
    let settings_len = read_u32(payload, 72) as usize;
    let expected = PAYLOAD_FIXED_BYTES
        .checked_add(name_len)
        .and_then(|size| size.checked_add(settings_len))
        .ok_or(StorageError::InvalidWorldMetadata(
            "metadata variable lengths overflow",
        ))?;
    if expected != payload.len() {
        return Err(StorageError::InvalidWorldMetadata(
            "metadata variable lengths mismatch",
        ));
    }

    let name_end = PAYLOAD_FIXED_BYTES + name_len;
    let name = std::str::from_utf8(&payload[PAYLOAD_FIXED_BYTES..name_end])
        .map_err(|_| StorageError::InvalidWorldMetadata("world name is not UTF-8"))?
        .to_owned();
    let metadata = WorldMetadata {
        generation: read_u64(envelope, 8),
        world_uuid: payload[16..32]
            .try_into()
            .expect("fixed 16-byte UUID slice"),
        name,
        seed: read_i64(payload, 32),
        generator_id: read_u32(payload, 40),
        generator_settings_version: read_u16(payload, 44),
        generator_settings: payload[name_end..].to_vec(),
        spawn_x: read_i32(payload, 48),
        spawn_y: read_i32(payload, 52),
        spawn_z: read_i32(payload, 56),
        time: read_i64(payload, 60),
        time_running: payload[5] != 0,
    };
    validate_metadata(&metadata)?;
    Ok(metadata)
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_i32(bytes: &mut [u8], offset: usize, value: i32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn put_i64(bytes: &mut [u8], offset: usize, value: i64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("u16 slice"))
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("u32 slice"))
}

fn read_i32(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("i32 slice"))
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("u64 slice"))
}

fn read_i64(bytes: &[u8], offset: usize) -> i64 {
    i64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("i64 slice"))
}
