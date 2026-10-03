use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, SyncSender};

use cobblestone_world::ChunkCoord;

use super::service::{LoadCommand, WorkerEvent};
use super::{LoadCompletion, LoadFailure, LoadWorkerError};
use crate::{RegionCoord, RegionFile, StorageError, StoredChunk, async_io::region_path};

pub(super) fn load_worker_main(
    root: PathBuf,
    world_uuid: [u8; 16],
    command_rx: Receiver<LoadCommand>,
    completion_tx: SyncSender<WorkerEvent>,
) {
    while let Ok(LoadCommand::Load(position)) = command_rx.recv() {
        let outcome = catch_unwind(AssertUnwindSafe(|| load_chunk(&root, world_uuid, position)));
        let completion = match outcome {
            Ok(Ok(Some(chunk))) => LoadCompletion::Loaded(chunk),
            Ok(Ok(None)) => LoadCompletion::Missing(position),
            Ok(Err(error)) => LoadCompletion::Failed(LoadFailure {
                position,
                error: LoadWorkerError::Storage(error),
            }),
            Err(_) => LoadCompletion::Failed(LoadFailure {
                position,
                error: LoadWorkerError::Panic,
            }),
        };

        if completion_tx
            .send(WorkerEvent::Completion(Box::new(completion)))
            .is_err()
        {
            return;
        }
    }

    let _ = completion_tx.send(WorkerEvent::Stopped);
}

fn load_chunk(
    root: &Path,
    world_uuid: [u8; 16],
    position: ChunkCoord,
) -> Result<Option<StoredChunk>, StorageError> {
    let region = RegionCoord::for_chunk(position);
    let Some(mut file) = RegionFile::open_existing(region_path(root, region), world_uuid, region)?
    else {
        return Ok(None);
    };

    file.load_chunk(position)
}
