use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{
    MAX_WORLD_METADATA_PAYLOAD_BYTES, WORLD_METADATA_ENVELOPE_BYTES, WORLD_METADATA_FILENAME,
    WorldMetadata, decode_world_metadata, encode_world_metadata,
};
use crate::{StorageError, sync_directory};

static TEMP_FILE_ID: AtomicU64 = AtomicU64::new(1);

pub(super) fn publish_metadata(
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

pub(super) fn read_metadata(path: impl AsRef<Path>) -> Result<WorldMetadata, StorageError> {
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
