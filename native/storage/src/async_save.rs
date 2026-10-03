use std::io;
use std::path::PathBuf;

use cobblestone_world::{ChunkCoord, ChunkSnapshot};
use thiserror::Error;

use crate::{CompressionPolicy, RegionCompactionResult, RegionCoord, RegionStats, StorageError};

mod service;
mod worker;

pub use service::AsyncSaveService;

pub const MAX_ASYNC_SAVE_WORKERS: usize = 32;
pub const MAX_ASYNC_SAVE_QUEUE_CAPACITY: usize = 65_536;
pub const MAX_ASYNC_SAVE_COMPLETION_CAPACITY: usize = 65_536;

#[derive(Debug, Clone)]
pub struct AsyncSaveConfig {
    pub root: PathBuf,
    pub world_uuid: [u8; 16],
    pub workers: usize,
    pub queue_capacity: usize,
    pub completion_capacity: usize,
    pub compression: CompressionPolicy,
}

impl AsyncSaveConfig {
    pub fn new(root: impl Into<PathBuf>, world_uuid: [u8; 16]) -> Self {
        Self {
            root: root.into(),
            world_uuid,
            workers: 1,
            queue_capacity: 1024,
            completion_capacity: 1024,
            compression: CompressionPolicy::Adaptive,
        }
    }
}

#[derive(Debug, Error)]
pub enum AsyncSaveBuildError {
    #[error("invalid async save configuration: {0}")]
    InvalidConfig(&'static str),
    #[error("failed to spawn async save worker: {0}")]
    Spawn(#[source] io::Error),
}

#[derive(Debug, Error)]
pub enum SaveSubmitError {
    #[error("async save queue is full")]
    Backpressure(ChunkSnapshot),
    #[error("async save worker is closed")]
    Closed(ChunkSnapshot),
}

impl SaveSubmitError {
    pub fn into_snapshot(self) -> ChunkSnapshot {
        match self {
            Self::Backpressure(snapshot) | Self::Closed(snapshot) => snapshot,
        }
    }
}

#[derive(Debug)]
pub struct SaveReceipt {
    pub position: ChunkCoord,
    pub terrain_revision: u64,
    pub light_revision: u64,
    pub lifecycle_flags: u8,
    pub region: RegionCoord,
    pub region_generation: u64,
    pub bytes_appended: u64,
    pub region_stats: RegionStats,
}

#[derive(Debug, Error)]
pub enum SaveWorkerError {
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("async storage worker panicked while processing a command")]
    Panic,
}

#[derive(Debug)]
pub struct SaveFailure {
    pub position: ChunkCoord,
    pub terrain_revision: u64,
    pub light_revision: u64,
    pub lifecycle_flags: u8,
    pub error: SaveWorkerError,
}

#[derive(Debug)]
pub enum SaveCompletion {
    Saved(SaveReceipt),
    Failed(SaveFailure),
}

#[derive(Debug)]
pub struct CompactionReceipt {
    pub region: RegionCoord,
    pub generation: u64,
    pub records: usize,
    pub bytes_reclaimed: u64,
    pub before: RegionStats,
    pub after: RegionStats,
}

impl From<(RegionCoord, RegionCompactionResult)> for CompactionReceipt {
    fn from((region, result): (RegionCoord, RegionCompactionResult)) -> Self {
        Self {
            region,
            generation: result.generation,
            records: result.records,
            bytes_reclaimed: result.bytes_reclaimed,
            before: result.before,
            after: result.after,
        }
    }
}

#[derive(Debug)]
pub struct CompactionFailure {
    pub region: RegionCoord,
    pub error: SaveWorkerError,
}

#[derive(Debug)]
pub enum CompactionCompletion {
    Compacted(CompactionReceipt),
    Failed(CompactionFailure),
}

impl SaveCompletion {
    pub fn position(&self) -> ChunkCoord {
        match self {
            Self::Saved(receipt) => receipt.position,
            Self::Failed(failure) => failure.position,
        }
    }

    pub fn terrain_revision(&self) -> u64 {
        match self {
            Self::Saved(receipt) => receipt.terrain_revision,
            Self::Failed(failure) => failure.terrain_revision,
        }
    }

    pub fn light_revision(&self) -> u64 {
        match self {
            Self::Saved(receipt) => receipt.light_revision,
            Self::Failed(failure) => failure.light_revision,
        }
    }

    pub fn lifecycle_flags(&self) -> u8 {
        match self {
            Self::Saved(receipt) => receipt.lifecycle_flags,
            Self::Failed(failure) => failure.lifecycle_flags,
        }
    }
}

#[derive(Debug, Error)]
pub enum CompactionSubmitError {
    #[error("async storage queue is full")]
    Backpressure(RegionCoord),
    #[error("async storage worker is closed")]
    Closed(RegionCoord),
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

    use super::{
        AsyncSaveConfig, AsyncSaveService, CompactionCompletion, SaveCompletion, SaveWorkerError,
    };
    use crate::{CompressionPolicy, RegionCoord, RegionFile, async_io::region_path};

    static TEMP_ID: AtomicU64 = AtomicU64::new(1);

    fn temp_root(name: &str) -> PathBuf {
        let id = TEMP_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "cobblestone-storage-{name}-{}-{id}",
            std::process::id()
        ))
    }

    fn snapshot(position: ChunkCoord, state: u16) -> cobblestone_world::ChunkSnapshot {
        let store = WorldStore::new();
        store.ensure_chunk(position, 1).unwrap();
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
    fn async_save_persists_exact_snapshot_and_receipt() {
        let root = temp_root("exact");
        let world_uuid = [0x2a; 16];
        let position = ChunkCoord::new(-17, 33);
        let expected = snapshot(position, 0x32);

        let mut config = AsyncSaveConfig::new(&root, world_uuid);
        config.workers = 2;
        config.queue_capacity = 4;
        config.completion_capacity = 4;
        config.compression = CompressionPolicy::Adaptive;

        let mut service = AsyncSaveService::start(config).unwrap();
        service.try_save(expected.clone()).unwrap();
        let completion = service
            .recv_completion_timeout(Duration::from_secs(5))
            .unwrap()
            .expect("save completion");
        let SaveCompletion::Saved(receipt) = completion else {
            panic!("async save failed");
        };
        assert_eq!(receipt.position, position);
        assert_eq!(receipt.terrain_revision, 1);
        assert_eq!(receipt.light_revision, 0);
        assert_eq!(receipt.lifecycle_flags, expected.lifecycle_flags());
        assert!(receipt.bytes_appended > 0);
        assert_eq!(receipt.region_stats.record_bytes, receipt.bytes_appended);
        assert_eq!(receipt.region_stats.live_bytes, receipt.bytes_appended);
        assert_eq!(receipt.region_stats.dead_bytes, 0);
        assert_eq!(receipt.region_stats.indexed_chunks, 1);

        assert!(service.shutdown().is_empty());

        let region = RegionCoord::for_chunk(position);
        let mut file =
            RegionFile::open_or_create(region_path(&root, region), world_uuid, region).unwrap();
        let loaded = file.load_chunk(position).unwrap().expect("stored chunk");
        assert_eq!(loaded.import.terrain_revision, 1);
        assert_eq!(loaded.import.light_revision, 0);
        assert_eq!(loaded.import.lifecycle_flags, expected.lifecycle_flags());
        assert_eq!(loaded.import.states[0], 0x32);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn compaction_runs_on_the_region_sharded_storage_worker() {
        let root = temp_root("compact");
        let world_uuid = [0x39; 16];
        let position = ChunkCoord::new(3, 7);
        let region = RegionCoord::for_chunk(position);

        let mut config = AsyncSaveConfig::new(&root, world_uuid);
        config.workers = 2;
        config.queue_capacity = 8;
        config.completion_capacity = 8;

        let mut service = AsyncSaveService::start(config).unwrap();
        service.try_save(snapshot(position, 0x21)).unwrap();
        service.try_save(snapshot(position, 0x45)).unwrap();

        for _ in 0..2 {
            let completion = service
                .recv_completion_timeout(Duration::from_secs(5))
                .unwrap()
                .expect("save completion");
            assert!(matches!(completion, SaveCompletion::Saved(_)));
        }

        service.try_compact(region).unwrap();
        let completion = service
            .recv_compaction_timeout(Duration::from_secs(5))
            .unwrap()
            .expect("compaction completion");
        let CompactionCompletion::Compacted(receipt) = completion else {
            panic!("scheduled compaction failed");
        };
        assert_eq!(receipt.region, region);
        assert!(receipt.bytes_reclaimed > 0);
        assert!(receipt.before.dead_bytes > 0);
        assert_eq!(receipt.after.dead_bytes, 0);
        assert_eq!(receipt.after.record_bytes, receipt.after.live_bytes);

        let mut reopened =
            RegionFile::open_existing(region_path(&root, region), world_uuid, region)
                .unwrap()
                .expect("compacted region");
        let loaded = reopened
            .load_chunk(position)
            .unwrap()
            .expect("stored chunk");
        assert_eq!(loaded.import.states[0], 0x45);

        assert!(service.shutdown().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn compaction_failure_is_reported_without_stopping_the_storage_worker() {
        let root = temp_root("compact-failure");
        let world_uuid = [0x27; 16];
        let missing_region = RegionCoord::new(9, -4);

        let mut config = AsyncSaveConfig::new(&root, world_uuid);
        config.queue_capacity = 4;
        config.completion_capacity = 4;
        let mut service = AsyncSaveService::start(config).unwrap();

        service.try_compact(missing_region).unwrap();
        let completion = service
            .recv_compaction_timeout(Duration::from_secs(5))
            .unwrap()
            .expect("compaction failure completion");
        let CompactionCompletion::Failed(failure) = completion else {
            panic!("missing region unexpectedly compacted");
        };
        assert_eq!(failure.region, missing_region);
        assert!(matches!(failure.error, SaveWorkerError::Storage(_)));

        let position = ChunkCoord::new(0, 0);
        service.try_save(snapshot(position, 0x56)).unwrap();
        let completion = service
            .recv_completion_timeout(Duration::from_secs(5))
            .unwrap()
            .expect("save completion after compaction failure");
        assert!(matches!(completion, SaveCompletion::Saved(_)));

        assert!(service.shutdown().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn shutdown_drains_all_accepted_save_jobs() {
        let root = temp_root("drain");
        let world_uuid = [0x51; 16];
        let mut config = AsyncSaveConfig::new(&root, world_uuid);
        config.workers = 2;
        config.queue_capacity = 16;
        config.completion_capacity = 2;

        let service = AsyncSaveService::start(config).unwrap();
        for x in 0..8 {
            service
                .try_save(snapshot(ChunkCoord::new(x, 0), 0x10 + x as u16))
                .unwrap();
        }

        let completions = service.shutdown();
        assert_eq!(completions.len(), 8);
        assert!(
            completions
                .iter()
                .all(|completion| matches!(completion, SaveCompletion::Saved(_)))
        );

        fs::remove_dir_all(root).unwrap();
    }
}
