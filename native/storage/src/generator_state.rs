use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::StorageError;

pub const GENERATOR_STATE_FILENAME: &str = "generator.cgs";
pub const GENERATOR_STATE_VERSION: u16 = 1;
pub const GENERATOR_STATE_HEADER_BYTES: usize = 48;
pub const MAX_GENERATOR_STATE_BYTES: usize = 64 * 1024 * 1024;

const MAGIC: &[u8; 4] = b"CBGS";
static TEMP_FILE_ID: AtomicU64 = AtomicU64::new(1);

pub fn write_generator_state(
    root: impl AsRef<Path>,
    world_uuid: [u8; 16],
    generator_id: u32,
    payload: &[u8],
) -> Result<(), StorageError> {
    if payload.len() > MAX_GENERATOR_STATE_BYTES {
        return Err(StorageError::GeneratorStateTooLarge {
            size: payload.len(),
            limit: MAX_GENERATOR_STATE_BYTES,
        });
    }

    let root = root.as_ref();
    let encoded = encode_generator_state(world_uuid, generator_id, payload)?;
    let destination = root.join(GENERATOR_STATE_FILENAME);
    let temporary = root.join(format!(
        "{}.{}.{}.tmp",
        GENERATOR_STATE_FILENAME,
        std::process::id(),
        TEMP_FILE_ID.fetch_add(1, Ordering::Relaxed),
    ));

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = match options.open(&temporary) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            std::fs::remove_file(&temporary)?;
            options.open(&temporary)?
        }
        Err(error) => return Err(error.into()),
    };
    file.write_all(&encoded)?;
    file.sync_data()?;
    drop(file);

    if let Err(error) = std::fs::rename(&temporary, &destination) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error.into());
    }
    File::open(root)?.sync_all()?;
    Ok(())
}

pub fn read_generator_state(
    root: impl AsRef<Path>,
    world_uuid: [u8; 16],
    generator_id: u32,
) -> Result<Option<Vec<u8>>, StorageError> {
    let path = root.as_ref().join(GENERATOR_STATE_FILENAME);
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };

    let length = file.metadata()?.len();
    let max = GENERATOR_STATE_HEADER_BYTES
        .checked_add(MAX_GENERATOR_STATE_BYTES)
        .expect("generator state maximum fits usize");
    if length > max as u64 {
        return Err(StorageError::GeneratorStateTooLarge {
            size: usize::try_from(length).unwrap_or(usize::MAX),
            limit: max,
        });
    }

    let mut encoded = Vec::with_capacity(length as usize);
    file.read_to_end(&mut encoded)?;
    decode_generator_state(&encoded, world_uuid, generator_id).map(Some)
}

fn encode_generator_state(
    world_uuid: [u8; 16],
    generator_id: u32,
    payload: &[u8],
) -> Result<Vec<u8>, StorageError> {
    let payload_len =
        u32::try_from(payload.len()).map_err(|_| StorageError::GeneratorStateTooLarge {
            size: payload.len(),
            limit: MAX_GENERATOR_STATE_BYTES,
        })?;

    let mut out = vec![0_u8; GENERATOR_STATE_HEADER_BYTES + payload.len()];
    out[0..4].copy_from_slice(MAGIC);
    out[4..6].copy_from_slice(&GENERATOR_STATE_VERSION.to_le_bytes());
    out[8..24].copy_from_slice(&world_uuid);
    out[24..28].copy_from_slice(&generator_id.to_le_bytes());
    out[28..32].copy_from_slice(&payload_len.to_le_bytes());
    out[32..36].copy_from_slice(&crc32c::crc32c(payload).to_le_bytes());
    let header_crc = crc32c::crc32c(&out[..36]);
    out[36..40].copy_from_slice(&header_crc.to_le_bytes());
    out[GENERATOR_STATE_HEADER_BYTES..].copy_from_slice(payload);
    Ok(out)
}

fn decode_generator_state(
    encoded: &[u8],
    expected_world_uuid: [u8; 16],
    expected_generator_id: u32,
) -> Result<Vec<u8>, StorageError> {
    if encoded.len() < GENERATOR_STATE_HEADER_BYTES {
        return Err(StorageError::InvalidGeneratorState("truncated header"));
    }
    if &encoded[0..4] != MAGIC {
        return Err(StorageError::InvalidGeneratorState("bad magic"));
    }
    if u16::from_le_bytes(encoded[4..6].try_into().expect("two-byte version"))
        != GENERATOR_STATE_VERSION
    {
        return Err(StorageError::InvalidGeneratorState("unsupported version"));
    }
    if encoded[6..8] != [0, 0] || encoded[40..48] != [0; 8] {
        return Err(StorageError::InvalidGeneratorState(
            "reserved bytes are nonzero",
        ));
    }
    if crc32c::crc32c(&encoded[..36])
        != u32::from_le_bytes(encoded[36..40].try_into().expect("four-byte header crc"))
    {
        return Err(StorageError::InvalidGeneratorState(
            "header checksum mismatch",
        ));
    }

    let world_uuid: [u8; 16] = encoded[8..24]
        .try_into()
        .expect("fixed generator-state uuid");
    if world_uuid != expected_world_uuid {
        return Err(StorageError::WorldMismatch);
    }
    let generator_id =
        u32::from_le_bytes(encoded[24..28].try_into().expect("four-byte generator id"));
    if generator_id != expected_generator_id {
        return Err(StorageError::GeneratorStateGeneratorMismatch {
            expected: expected_generator_id,
            actual: generator_id,
        });
    }

    let payload_len = u32::from_le_bytes(
        encoded[28..32]
            .try_into()
            .expect("four-byte payload length"),
    ) as usize;
    if payload_len > MAX_GENERATOR_STATE_BYTES {
        return Err(StorageError::GeneratorStateTooLarge {
            size: payload_len,
            limit: MAX_GENERATOR_STATE_BYTES,
        });
    }
    let expected_len = GENERATOR_STATE_HEADER_BYTES
        .checked_add(payload_len)
        .ok_or(StorageError::InvalidGeneratorState("length overflow"))?;
    if encoded.len() != expected_len {
        return Err(StorageError::InvalidGeneratorState(
            "payload length mismatch",
        ));
    }
    let payload = &encoded[GENERATOR_STATE_HEADER_BYTES..];
    if crc32c::crc32c(payload)
        != u32::from_le_bytes(encoded[32..36].try_into().expect("four-byte payload crc"))
    {
        return Err(StorageError::InvalidGeneratorState(
            "payload checksum mismatch",
        ));
    }
    Ok(payload.to_vec())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static TEMP_ID: AtomicU64 = AtomicU64::new(1);

    fn temp_root(name: &str) -> PathBuf {
        let id = TEMP_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "cobblestone-generator-state-{name}-{}-{id}",
            std::process::id()
        ))
    }

    #[test]
    fn atomic_generator_state_round_trips_and_replaces() {
        let root = temp_root("roundtrip");
        fs::create_dir_all(&root).unwrap();
        let uuid = [0x38; 16];

        assert_eq!(read_generator_state(&root, uuid, 1).unwrap(), None);
        write_generator_state(&root, uuid, 1, b"first").unwrap();
        assert_eq!(
            read_generator_state(&root, uuid, 1).unwrap(),
            Some(b"first".to_vec())
        );

        write_generator_state(&root, uuid, 1, b"second-state").unwrap();
        assert_eq!(
            read_generator_state(&root, uuid, 1).unwrap(),
            Some(b"second-state".to_vec())
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn generator_state_rejects_wrong_world_generator_and_corruption() {
        let root = temp_root("reject");
        fs::create_dir_all(&root).unwrap();
        let uuid = [0x51; 16];
        write_generator_state(&root, uuid, 1, b"payload").unwrap();

        assert!(matches!(
            read_generator_state(&root, [0x52; 16], 1),
            Err(StorageError::WorldMismatch)
        ));
        assert!(matches!(
            read_generator_state(&root, uuid, 2),
            Err(StorageError::GeneratorStateGeneratorMismatch {
                expected: 2,
                actual: 1
            })
        ));

        let path = root.join(GENERATOR_STATE_FILENAME);
        let mut bytes = fs::read(&path).unwrap();
        *bytes.last_mut().unwrap() ^= 0xff;
        fs::write(&path, bytes).unwrap();
        assert!(matches!(
            read_generator_state(&root, uuid, 1),
            Err(StorageError::InvalidGeneratorState(
                "payload checksum mismatch"
            ))
        ));
        fs::remove_dir_all(root).unwrap();
    }
}
