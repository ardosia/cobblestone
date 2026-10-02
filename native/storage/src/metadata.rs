use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{STORAGE_FORMAT_VERSION, StorageError};

pub const WORLD_METADATA_FILENAME: &str = "world.cwm";
pub const WORLD_METADATA_ENVELOPE_BYTES: usize = 32;
pub const WORLD_METADATA_PAYLOAD_VERSION: u16 = 1;
pub const MAX_WORLD_METADATA_PAYLOAD_BYTES: usize = 1024 * 1024;
pub const MAX_WORLD_NAME_BYTES: usize = 4096;
pub const MAX_GENERATOR_SETTINGS_BYTES: usize = 512 * 1024;

pub const TARGET_VERSION_MAJOR: u8 = 0;
pub const TARGET_VERSION_MINOR: u8 = 15;
pub const TARGET_VERSION_PATCH: u8 = 10;
pub const TARGET_GAME_PROTOCOL: u32 = 84;
pub const TARGET_RAKNET_PROTOCOL: u32 = 8;

const WORLD_METADATA_MAGIC: &[u8; 4] = b"CBWM";
const PAYLOAD_FIXED_BYTES: usize = 76;
static TEMP_FILE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WorldMetadata {
    pub generation: u64,
    pub world_uuid: [u8; 16],
    pub name: String,
    pub seed: i64,
    pub generator_id: u32,
    pub generator_settings_version: u16,
    pub generator_settings: Vec<u8>,
    pub spawn_x: i32,
    pub spawn_y: i32,
    pub spawn_z: i32,
    pub time: i64,
    pub time_running: bool,
}

impl WorldMetadata {
    pub fn new(
        world_uuid: [u8; 16],
        name: impl Into<String>,
        seed: i64,
        generator_id: u32,
        generator_settings_version: u16,
        generator_settings: Vec<u8>,
    ) -> Self {
        Self {
            generation: 1,
            world_uuid,
            name: name.into(),
            seed,
            generator_id,
            generator_settings_version,
            generator_settings,
            spawn_x: 0,
            spawn_y: 0,
            spawn_z: 0,
            time: 0,
            time_running: true,
        }
    }
}

#[derive(Debug)]
pub struct WorldDirectory {
    root: PathBuf,
    metadata: WorldMetadata,
}

impl WorldDirectory {
    pub fn create(root: impl AsRef<Path>, metadata: WorldMetadata) -> Result<Self, StorageError> {
        validate_metadata(&metadata)?;
        if metadata.generation != 1 {
            return Err(StorageError::InvalidWorldMetadata(
                "new world metadata generation must be 1",
            ));
        }

        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(root.join("regions"))?;
        let metadata_path = root.join(WORLD_METADATA_FILENAME);
        if metadata_path.exists() {
            return Err(StorageError::WorldMetadataAlreadyExists);
        }

        publish_metadata(&root, &metadata, true)?;
        Ok(Self { root, metadata })
    }

    pub fn open(root: impl AsRef<Path>) -> Result<Self, StorageError> {
        let root = root.as_ref().to_path_buf();
        let metadata = read_metadata(root.join(WORLD_METADATA_FILENAME))?;
        validate_metadata(&metadata)?;
        std::fs::create_dir_all(root.join("regions"))?;

        Ok(Self { root, metadata })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn regions_dir(&self) -> PathBuf {
        self.root.join("regions")
    }

    pub fn metadata(&self) -> &WorldMetadata {
        &self.metadata
    }

    pub fn world_uuid(&self) -> [u8; 16] {
        self.metadata.world_uuid
    }

    pub fn replace_metadata(
        &mut self,
        mut metadata: WorldMetadata,
    ) -> Result<&WorldMetadata, StorageError> {
        if metadata.world_uuid != self.metadata.world_uuid {
            return Err(StorageError::WorldMismatch);
        }
        metadata.generation = self
            .metadata
            .generation
            .checked_add(1)
            .ok_or(StorageError::MetadataGenerationExhausted)?;
        validate_metadata(&metadata)?;

        publish_metadata(&self.root, &metadata, false)?;
        self.metadata = metadata;
        Ok(&self.metadata)
    }
}

fn validate_metadata(metadata: &WorldMetadata) -> Result<(), StorageError> {
    let name = metadata.name.as_bytes();
    if name.is_empty() {
        return Err(StorageError::InvalidWorldMetadata(
            "world name cannot be empty",
        ));
    }
    if name.len() > MAX_WORLD_NAME_BYTES {
        return Err(StorageError::WorldMetadataTooLarge {
            size: name.len(),
            limit: MAX_WORLD_NAME_BYTES,
        });
    }
    if metadata.generator_settings.len() > MAX_GENERATOR_SETTINGS_BYTES {
        return Err(StorageError::WorldMetadataTooLarge {
            size: metadata.generator_settings.len(),
            limit: MAX_GENERATOR_SETTINGS_BYTES,
        });
    }
    if metadata.generation == 0 {
        return Err(StorageError::InvalidWorldMetadata(
            "metadata generation cannot be zero",
        ));
    }
    if !(0..=127).contains(&metadata.spawn_y) {
        return Err(StorageError::InvalidWorldMetadata(
            "spawn y is outside the fixed-target world height",
        ));
    }
    Ok(())
}

fn publish_metadata(
    root: &Path,
    metadata: &WorldMetadata,
    create_only: bool,
) -> Result<(), StorageError> {
    let encoded = encode_world_metadata(metadata)?;
    let destination = root.join(WORLD_METADATA_FILENAME);
    let temporary = root.join(format!(
        "{}.{}.{}.tmp",
        WORLD_METADATA_FILENAME,
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

    if create_only {
        match std::fs::hard_link(&temporary, &destination) {
            Ok(()) => {
                std::fs::remove_file(&temporary)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let _ = std::fs::remove_file(&temporary);
                return Err(StorageError::WorldMetadataAlreadyExists);
            }
            Err(error) => {
                let _ = std::fs::remove_file(&temporary);
                return Err(error.into());
            }
        }
    } else if let Err(error) = std::fs::rename(&temporary, &destination) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error.into());
    }
    sync_directory(root)?;
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), StorageError> {
    File::open(path)?.sync_all()?;
    Ok(())
}

pub fn encode_world_metadata(metadata: &WorldMetadata) -> Result<Vec<u8>, StorageError> {
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

pub fn decode_world_metadata(encoded: &[u8]) -> Result<WorldMetadata, StorageError> {
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

fn read_metadata(path: impl AsRef<Path>) -> Result<WorldMetadata, StorageError> {
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    let max = WORLD_METADATA_ENVELOPE_BYTES + MAX_WORLD_METADATA_PAYLOAD_BYTES;
    if length > max as u64 {
        return Err(StorageError::WorldMetadataTooLarge {
            size: usize::try_from(length).unwrap_or(usize::MAX),
            limit: max,
        });
    }

    let mut encoded = Vec::with_capacity(length as usize);
    file.read_to_end(&mut encoded)?;
    decode_world_metadata(&encoded)
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{
        WORLD_METADATA_FILENAME, WorldDirectory, WorldMetadata, decode_world_metadata,
        encode_world_metadata,
    };
    use crate::StorageError;

    static TEMP_ID: AtomicU64 = AtomicU64::new(1);

    fn temp_root(name: &str) -> PathBuf {
        let id = TEMP_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "cobblestone-world-meta-{name}-{}-{id}",
            std::process::id()
        ))
    }

    fn metadata() -> WorldMetadata {
        let mut metadata = WorldMetadata::new(
            [0x44; 16],
            "world",
            -123456789,
            2,
            1,
            b"2;7,2x3,2;1;".to_vec(),
        );
        metadata.spawn_x = 128;
        metadata.spawn_y = 64;
        metadata.spawn_z = -128;
        metadata.time = 6000;
        metadata.time_running = false;
        metadata
    }

    #[test]
    fn metadata_codec_round_trips_exact_semantics() {
        let expected = metadata();
        let encoded = encode_world_metadata(&expected).unwrap();
        let decoded = decode_world_metadata(&encoded).unwrap();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn world_directory_create_reopen_and_replace_are_atomic_contracts() {
        let root = temp_root("directory");
        let expected = metadata();

        let mut directory = WorldDirectory::create(&root, expected.clone()).unwrap();
        assert_eq!(directory.metadata(), &expected);
        assert!(directory.regions_dir().is_dir());

        let reopened = WorldDirectory::open(&root).unwrap();
        assert_eq!(reopened.metadata(), &expected);

        let mut next = expected.clone();
        next.name = "renamed".to_owned();
        next.time = 12000;
        let committed = directory.replace_metadata(next).unwrap();
        assert_eq!(committed.generation, 2);
        assert_eq!(committed.name, "renamed");

        let reopened = WorldDirectory::open(&root).unwrap();
        assert_eq!(reopened.metadata().generation, 2);
        assert_eq!(reopened.metadata().name, "renamed");
        assert_eq!(reopened.metadata().time, 12000);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corrupt_metadata_never_silently_opens() {
        let root = temp_root("corrupt");
        WorldDirectory::create(&root, metadata()).unwrap();
        let path = root.join(WORLD_METADATA_FILENAME);

        let mut encoded = fs::read(&path).unwrap();
        let last = encoded.len() - 1;
        encoded[last] ^= 0x80;
        fs::write(&path, encoded).unwrap();

        assert!(matches!(
            WorldDirectory::open(&root),
            Err(StorageError::InvalidWorldMetadata(
                "metadata payload checksum mismatch"
            ))
        ));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn replacement_cannot_change_world_identity() {
        let root = temp_root("identity");
        let expected = metadata();
        let mut directory = WorldDirectory::create(&root, expected.clone()).unwrap();
        let mut replacement = expected;
        replacement.world_uuid = [0x99; 16];

        assert!(matches!(
            directory.replace_metadata(replacement),
            Err(StorageError::WorldMismatch)
        ));

        fs::remove_dir_all(root).unwrap();
    }
}
