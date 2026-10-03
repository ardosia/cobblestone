use std::io;
use std::path::PathBuf;

use cobblestone_world::ChunkCoord;
use thiserror::Error;

use crate::{StorageError, StoredChunk};

mod service;
mod worker;

pub use service::AsyncLoadService;

pub const MAX_ASYNC_LOAD_WORKERS: usize = 32;
pub const MAX_ASYNC_LOAD_QUEUE_CAPACITY: usize = 65_536;
pub const MAX_ASYNC_LOAD_COMPLETION_CAPACITY: usize = 65_536;

#[derive(Debug, Clone)]
pub struct AsyncLoadConfig {
    pub root: PathBuf,
    pub world_uuid: [u8; 16],
    pub workers: usize,
    pub queue_capacity: usize,
    pub completion_capacity: usize,
}

impl AsyncLoadConfig {
    pub fn new(root: impl Into<PathBuf>, world_uuid: [u8; 16]) -> Self {
        Self {
            root: root.into(),
            world_uuid,
            workers: 1,
            queue_capacity: 1024,
            completion_capacity: 1024,
        }
    }
}

#[derive(Debug, Error)]
pub enum AsyncLoadBuildError {
    #[error("invalid async load configuration: {0}")]
    InvalidConfig(&'static str),
    #[error("failed to spawn async load worker: {0}")]
    Spawn(#[source] io::Error),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum LoadRequestState {
    Queued,
    Joined,
}

#[derive(Debug, Error)]
pub enum LoadSubmitError {
    #[error("async load queue is full for chunk {0:?}")]
    Backpressure(ChunkCoord),
    #[error("async load worker is closed for chunk {0:?}")]
    Closed(ChunkCoord),
}

#[derive(Debug, Error)]
pub enum LoadWorkerError {
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("async load worker panicked while reading a chunk")]
    Panic,
}

#[derive(Debug)]
pub struct LoadFailure {
    pub position: ChunkCoord,
    pub error: LoadWorkerError,
}

#[derive(Debug)]
pub enum LoadCompletion {
    Loaded(StoredChunk),
    Missing(ChunkCoord),
    Failed(LoadFailure),
}

impl LoadCompletion {
    pub fn position(&self) -> ChunkCoord {
        match self {
            Self::Loaded(chunk) => chunk.position,
            Self::Missing(position) => *position,
            Self::Failed(failure) => failure.position,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use cobblestone_world::{
        CHUNK_LIFECYCLE_GENERATED, CHUNK_LIFECYCLE_POPULATED, ChunkCoord, ChunkPatch, WorldStore,
    };

    use super::{AsyncLoadConfig, AsyncLoadService, LoadCompletion, LoadRequestState};
    use crate::async_io::region_path;
    use crate::{CompressionPolicy, RegionCoord, RegionFile};

    static TEMP_ID: AtomicU64 = AtomicU64::new(1);

    fn temp_root(name: &str) -> PathBuf {
        let id = TEMP_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "cobblestone-load-{name}-{}-{id}",
            std::process::id()
        ))
    }

    fn snapshot(position: ChunkCoord, state: u16) -> cobblestone_world::ChunkSnapshot {
        let store = WorldStore::new();
        store.ensure_chunk(position, 1);
        let flags = CHUNK_LIFECYCLE_GENERATED | CHUNK_LIFECYCLE_POPULATED;
        store.set_lifecycle_flags(position, flags).unwrap();
        store
            .apply_patch(
                position,
                ChunkPatch {
                    expected_terrain_revision: 0,
                    next_terrain_revision: 1,
                    expected_light_revision: 0,
                    next_light_revision: 0,
                    blocks: vec![(0, state)],
                    ..ChunkPatch::default()
                },
            )
            .unwrap();
        store.snapshot(position).unwrap()
    }

    #[test]
    fn duplicate_requests_join_one_in_flight_disk_read() {
        let root = temp_root("dedup");
        let world_uuid = [0x63; 16];
        let position = ChunkCoord::new(17, -33);
        let expected = snapshot(position, 0x42);
        let region = RegionCoord::for_chunk(position);

        let mut file =
            RegionFile::open_or_create(region_path(&root, region), world_uuid, region).unwrap();
        file.save_chunk(&expected, CompressionPolicy::Adaptive)
            .unwrap();
        drop(file);

        let mut config = AsyncLoadConfig::new(&root, world_uuid);
        config.workers = 2;
        config.queue_capacity = 4;
        config.completion_capacity = 4;
        let mut service = AsyncLoadService::start(config).unwrap();

        assert_eq!(service.request(position).unwrap(), LoadRequestState::Queued);
        assert_eq!(service.request(position).unwrap(), LoadRequestState::Joined);
        assert_eq!(service.in_flight(), 1);

        let completion = service
            .recv_completion_timeout(Duration::from_secs(5))
            .unwrap()
            .expect("load completion");
        let LoadCompletion::Loaded(loaded) = completion else {
            panic!("expected loaded chunk");
        };
        assert_eq!(loaded.position, position);
        assert_eq!(loaded.import.states[0], 0x42);
        assert_eq!(service.in_flight(), 0);

        assert_eq!(service.request(position).unwrap(), LoadRequestState::Queued);
        let drained = service.shutdown();
        assert_eq!(drained.len(), 1);
        assert!(matches!(drained[0], LoadCompletion::Loaded(_)));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_chunk_does_not_create_a_region_file() {
        let root = temp_root("missing");
        let world_uuid = [0x71; 16];
        let position = ChunkCoord::new(-64, 80);
        let region = RegionCoord::for_chunk(position);
        let path = region_path(&root, region);

        let mut service = AsyncLoadService::start(AsyncLoadConfig::new(&root, world_uuid)).unwrap();
        assert_eq!(service.request(position).unwrap(), LoadRequestState::Queued);
        let completion = service
            .recv_completion_timeout(Duration::from_secs(5))
            .unwrap()
            .expect("missing completion");
        assert!(matches!(completion, LoadCompletion::Missing(found) if found == position));
        assert!(!path.exists());

        assert!(service.shutdown().is_empty());
        let _ = fs::remove_dir_all(root);
    }
}
