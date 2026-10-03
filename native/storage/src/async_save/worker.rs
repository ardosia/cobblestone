use std::collections::HashMap;
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, SyncSender};

use cobblestone_world::ChunkSnapshot;

use super::service::{SaveCommand, WorkerEvent};
use super::{
    CompactionCompletion, CompactionFailure, CompactionReceipt, SaveCompletion, SaveFailure,
    SaveReceipt, SaveWorkerError,
};
use crate::{CompressionPolicy, RegionCoord, RegionFile, StorageError, async_io::region_path};

pub(super) fn save_worker_main(
    root: PathBuf,
    world_uuid: [u8; 16],
    compression: CompressionPolicy,
    command_rx: Receiver<SaveCommand>,
    completion_tx: SyncSender<WorkerEvent>,
) {
    let mut regions = HashMap::<RegionCoord, RegionFile>::new();

    while let Ok(command) = command_rx.recv() {
        match command {
            SaveCommand::Save(snapshot) => {
                let position = snapshot.position();
                let terrain_revision = snapshot.terrain_revision();
                let light_revision = snapshot.light_revision();
                let lifecycle_flags = snapshot.lifecycle_flags();

                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    save_snapshot(&root, world_uuid, compression, &mut regions, &snapshot)
                }));

                let completion = match outcome {
                    Ok(Ok(receipt)) => SaveCompletion::Saved(receipt),
                    Ok(Err(error)) => SaveCompletion::Failed(SaveFailure {
                        position,
                        terrain_revision,
                        light_revision,
                        lifecycle_flags,
                        error: SaveWorkerError::Storage(error),
                    }),
                    Err(_) => SaveCompletion::Failed(SaveFailure {
                        position,
                        terrain_revision,
                        light_revision,
                        lifecycle_flags,
                        error: SaveWorkerError::Panic,
                    }),
                };

                if completion_tx
                    .send(WorkerEvent::Completion(completion))
                    .is_err()
                {
                    return;
                }
            }
            SaveCommand::Compact(region) => {
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    compact_region(&root, world_uuid, &mut regions, region)
                }));
                let completion = match outcome {
                    Ok(Ok(receipt)) => CompactionCompletion::Compacted(receipt),
                    Ok(Err(error)) => CompactionCompletion::Failed(CompactionFailure {
                        region,
                        error: SaveWorkerError::Storage(error),
                    }),
                    Err(_) => CompactionCompletion::Failed(CompactionFailure {
                        region,
                        error: SaveWorkerError::Panic,
                    }),
                };

                if completion_tx
                    .send(WorkerEvent::Compaction(completion))
                    .is_err()
                {
                    return;
                }
            }
        }
    }

    let _ = completion_tx.send(WorkerEvent::Stopped);
}

fn save_snapshot(
    root: &Path,
    world_uuid: [u8; 16],
    compression: CompressionPolicy,
    regions: &mut HashMap<RegionCoord, RegionFile>,
    snapshot: &ChunkSnapshot,
) -> Result<SaveReceipt, StorageError> {
    let position = snapshot.position();
    let region = RegionCoord::for_chunk(position);
    let file = match regions.entry(region) {
        std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
        std::collections::hash_map::Entry::Vacant(entry) => {
            let file = RegionFile::open_or_create(region_path(root, region), world_uuid, region)?;
            entry.insert(file)
        }
    };

    let result = file.save_chunk(snapshot, compression)?;

    Ok(SaveReceipt {
        position,
        terrain_revision: snapshot.terrain_revision(),
        light_revision: snapshot.light_revision(),
        lifecycle_flags: snapshot.lifecycle_flags(),
        region,
        region_generation: result.generation,
        bytes_appended: result.bytes_appended,
        region_stats: result.stats,
    })
}

fn compact_region(
    root: &Path,
    world_uuid: [u8; 16],
    regions: &mut HashMap<RegionCoord, RegionFile>,
    region: RegionCoord,
) -> Result<CompactionReceipt, StorageError> {
    let file = match regions.entry(region) {
        std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
        std::collections::hash_map::Entry::Vacant(entry) => {
            let path = region_path(root, region);
            let file = RegionFile::open_existing(path, world_uuid, region)?.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "region disappeared before scheduled compaction",
                )
            })?;
            entry.insert(file)
        }
    };
    let result = file.compact()?;
    Ok((region, result).into())
}
