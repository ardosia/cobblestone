use std::path::{Path, PathBuf};

use crate::StorageError;

mod codec;
mod io;

pub use codec::{decode_world_metadata, encode_world_metadata};
use io::{publish_metadata, read_metadata};

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

pub(super) fn validate_metadata(metadata: &WorldMetadata) -> Result<(), StorageError> {
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
