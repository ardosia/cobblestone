use std::collections::{HashMap, VecDeque};
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{
    Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError, sync_channel,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cobblestone_world::{ChunkCoord, ChunkSnapshot};
use thiserror::Error;

use crate::{
    CompressionPolicy, RegionCompactionResult, RegionCoord, RegionFile, RegionStats, StorageError,
};

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

enum SaveCommand {
    Save(ChunkSnapshot),
    Compact(RegionCoord),
}

enum WorkerEvent {
    Completion(SaveCompletion),
    Compaction(CompactionCompletion),
    Stopped,
}

struct SaveWorker {
    sender: Option<SyncSender<SaveCommand>>,
    join: Option<JoinHandle<()>>,
}

pub struct AsyncSaveService {
    workers: Vec<SaveWorker>,
    completion_rx: Option<Receiver<WorkerEvent>>,
    pending_completions: VecDeque<SaveCompletion>,
    pending_compactions: VecDeque<CompactionCompletion>,
    observed_stopped: usize,
}

impl AsyncSaveService {
    pub fn start(config: AsyncSaveConfig) -> Result<Self, AsyncSaveBuildError> {
        validate_config(&config)?;

        let (completion_tx, completion_rx) = sync_channel(config.completion_capacity);
        let mut workers: Vec<SaveWorker> = Vec::with_capacity(config.workers);

        for worker_index in 0..config.workers {
            let (command_tx, command_rx) = sync_channel(config.queue_capacity);
            let completion_tx = completion_tx.clone();
            let root = config.root.clone();
            let world_uuid = config.world_uuid;
            let compression = config.compression;
            let spawn = thread::Builder::new()
                .name(format!("cobblestone-storage-save-{worker_index}"))
                .spawn(move || {
                    save_worker_main(root, world_uuid, compression, command_rx, completion_tx);
                });

            let join = match spawn {
                Ok(join) => join,
                Err(error) => {
                    for worker in &mut workers {
                        worker.sender.take();
                    }
                    drop(completion_rx);
                    for worker in &mut workers {
                        if let Some(join) = worker.join.take() {
                            let _ = join.join();
                        }
                    }
                    return Err(AsyncSaveBuildError::Spawn(error));
                }
            };

            workers.push(SaveWorker {
                sender: Some(command_tx),
                join: Some(join),
            });
        }
        drop(completion_tx);

        Ok(Self {
            workers,
            completion_rx: Some(completion_rx),
            pending_completions: VecDeque::new(),
            pending_compactions: VecDeque::new(),
            observed_stopped: 0,
        })
    }

    pub fn try_save(&self, snapshot: ChunkSnapshot) -> Result<(), SaveSubmitError> {
        let worker = route_worker(snapshot.position(), self.workers.len());
        let Some(sender) = self.workers[worker].sender.as_ref() else {
            return Err(SaveSubmitError::Closed(snapshot));
        };

        match sender.try_send(SaveCommand::Save(snapshot)) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(SaveCommand::Save(snapshot))) => {
                Err(SaveSubmitError::Backpressure(snapshot))
            }
            Err(TrySendError::Disconnected(SaveCommand::Save(snapshot))) => {
                Err(SaveSubmitError::Closed(snapshot))
            }
            Err(
                TrySendError::Full(SaveCommand::Compact(_))
                | TrySendError::Disconnected(SaveCommand::Compact(_)),
            ) => {
                unreachable!("try_save submitted only a save command")
            }
        }
    }

    pub fn try_compact(&self, region: RegionCoord) -> Result<(), CompactionSubmitError> {
        let worker = route_region_worker(region, self.workers.len());
        let Some(sender) = self.workers[worker].sender.as_ref() else {
            return Err(CompactionSubmitError::Closed(region));
        };

        match sender.try_send(SaveCommand::Compact(region)) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(SaveCommand::Compact(region))) => {
                Err(CompactionSubmitError::Backpressure(region))
            }
            Err(TrySendError::Disconnected(SaveCommand::Compact(region))) => {
                Err(CompactionSubmitError::Closed(region))
            }
            Err(
                TrySendError::Full(SaveCommand::Save(_))
                | TrySendError::Disconnected(SaveCommand::Save(_)),
            ) => {
                unreachable!("try_compact submitted only a compaction command")
            }
        }
    }

    pub fn try_recv_completion(&mut self) -> Option<SaveCompletion> {
        if let Some(completion) = self.pending_completions.pop_front() {
            return Some(completion);
        }

        loop {
            let receiver = self.completion_rx.as_ref()?;
            match receiver.try_recv() {
                Ok(WorkerEvent::Completion(completion)) => return Some(completion),
                Ok(WorkerEvent::Compaction(completion)) => {
                    self.pending_compactions.push_back(completion);
                }
                Ok(WorkerEvent::Stopped) => {
                    self.observed_stopped = self.observed_stopped.saturating_add(1);
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return None,
            }
        }
    }

    pub fn try_recv_compaction(&mut self) -> Option<CompactionCompletion> {
        if let Some(completion) = self.pending_compactions.pop_front() {
            return Some(completion);
        }

        loop {
            let receiver = self.completion_rx.as_ref()?;
            match receiver.try_recv() {
                Ok(WorkerEvent::Completion(completion)) => {
                    self.pending_completions.push_back(completion);
                }
                Ok(WorkerEvent::Compaction(completion)) => return Some(completion),
                Ok(WorkerEvent::Stopped) => {
                    self.observed_stopped = self.observed_stopped.saturating_add(1);
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return None,
            }
        }
    }

    pub fn recv_completion_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<SaveCompletion>, RecvTimeoutError> {
        if let Some(completion) = self.pending_completions.pop_front() {
            return Ok(Some(completion));
        }

        loop {
            let Some(receiver) = self.completion_rx.as_ref() else {
                return Ok(None);
            };
            match receiver.recv_timeout(timeout)? {
                WorkerEvent::Completion(completion) => return Ok(Some(completion)),
                WorkerEvent::Compaction(completion) => {
                    self.pending_compactions.push_back(completion);
                }
                WorkerEvent::Stopped => {
                    self.observed_stopped = self.observed_stopped.saturating_add(1);
                    if self.observed_stopped == self.workers.len() {
                        return Ok(None);
                    }
                }
            }
        }
    }

    pub fn recv_compaction_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<CompactionCompletion>, RecvTimeoutError> {
        if let Some(completion) = self.pending_compactions.pop_front() {
            return Ok(Some(completion));
        }

        loop {
            let Some(receiver) = self.completion_rx.as_ref() else {
                return Ok(None);
            };
            match receiver.recv_timeout(timeout)? {
                WorkerEvent::Completion(completion) => {
                    self.pending_completions.push_back(completion);
                }
                WorkerEvent::Compaction(completion) => return Ok(Some(completion)),
                WorkerEvent::Stopped => {
                    self.observed_stopped = self.observed_stopped.saturating_add(1);
                    if self.observed_stopped == self.workers.len() {
                        return Ok(None);
                    }
                }
            }
        }
    }

    pub fn shutdown(mut self) -> Vec<SaveCompletion> {
        self.shutdown_inner()
    }

    fn shutdown_inner(&mut self) -> Vec<SaveCompletion> {
        for worker in &mut self.workers {
            worker.sender.take();
        }

        let mut completions: Vec<SaveCompletion> = self.pending_completions.drain(..).collect();
        if let Some(receiver) = self.completion_rx.take() {
            while self.observed_stopped < self.workers.len() {
                match receiver.recv() {
                    Ok(WorkerEvent::Completion(completion)) => completions.push(completion),
                    Ok(WorkerEvent::Compaction(completion)) => {
                        self.pending_compactions.push_back(completion);
                    }
                    Ok(WorkerEvent::Stopped) => {
                        self.observed_stopped = self.observed_stopped.saturating_add(1);
                    }
                    Err(_) => break,
                }
            }
        }

        for worker in &mut self.workers {
            if let Some(join) = worker.join.take() {
                let _ = join.join();
            }
        }

        completions
    }
}

impl Drop for AsyncSaveService {
    fn drop(&mut self) {
        let _ = self.shutdown_inner();
    }
}

fn validate_config(config: &AsyncSaveConfig) -> Result<(), AsyncSaveBuildError> {
    if config.workers == 0 || config.workers > MAX_ASYNC_SAVE_WORKERS {
        return Err(AsyncSaveBuildError::InvalidConfig(
            "workers must be in range 1..=32",
        ));
    }
    if config.queue_capacity == 0 || config.queue_capacity > MAX_ASYNC_SAVE_QUEUE_CAPACITY {
        return Err(AsyncSaveBuildError::InvalidConfig(
            "queue capacity must be in range 1..=65536",
        ));
    }
    if config.completion_capacity == 0
        || config.completion_capacity > MAX_ASYNC_SAVE_COMPLETION_CAPACITY
    {
        return Err(AsyncSaveBuildError::InvalidConfig(
            "completion capacity must be in range 1..=65536",
        ));
    }
    Ok(())
}

fn route_worker(position: ChunkCoord, workers: usize) -> usize {
    route_region_worker(RegionCoord::for_chunk(position), workers)
}

fn route_region_worker(region: RegionCoord, workers: usize) -> usize {
    let x = region.x as i64 as u64;
    let z = region.z as i64 as u64;
    let mixed = x.wrapping_mul(0x9e37_79b9_7f4a_7c15).rotate_left(17)
        ^ z.wrapping_mul(0xc2b2_ae3d_27d4_eb4f);
    mixed as usize % workers
}

fn region_path(root: &Path, region: RegionCoord) -> PathBuf {
    root.join("regions")
        .join(format!("r.{}.{}.cwr", region.x, region.z))
}

fn save_worker_main(
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
        region_path,
    };
    use crate::{CompressionPolicy, RegionCoord, RegionFile};

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
