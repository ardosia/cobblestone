use std::collections::HashSet;
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{
    Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError, sync_channel,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cobblestone_core::ChunkCoord;
use thiserror::Error;

use crate::{RegionCoord, RegionFile, StorageError, StoredChunk};

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

enum LoadCommand {
    Load(ChunkCoord),
}

enum WorkerEvent {
    Completion(Box<LoadCompletion>),
    Stopped,
}

struct LoadWorker {
    sender: Option<SyncSender<LoadCommand>>,
    join: Option<JoinHandle<()>>,
}

pub struct AsyncLoadService {
    workers: Vec<LoadWorker>,
    completion_rx: Option<Receiver<WorkerEvent>>,
    in_flight: HashSet<ChunkCoord>,
    observed_stopped: usize,
}

impl AsyncLoadService {
    pub fn start(config: AsyncLoadConfig) -> Result<Self, AsyncLoadBuildError> {
        validate_config(&config)?;

        let (completion_tx, completion_rx) = sync_channel(config.completion_capacity);
        let mut workers: Vec<LoadWorker> = Vec::with_capacity(config.workers);

        for worker_index in 0..config.workers {
            let (command_tx, command_rx) = sync_channel(config.queue_capacity);
            let completion_tx = completion_tx.clone();
            let root = config.root.clone();
            let world_uuid = config.world_uuid;
            let spawn = thread::Builder::new()
                .name(format!("cobblestone-storage-load-{worker_index}"))
                .spawn(move || {
                    load_worker_main(root, world_uuid, command_rx, completion_tx);
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
                    return Err(AsyncLoadBuildError::Spawn(error));
                }
            };

            workers.push(LoadWorker {
                sender: Some(command_tx),
                join: Some(join),
            });
        }
        drop(completion_tx);

        Ok(Self {
            workers,
            completion_rx: Some(completion_rx),
            in_flight: HashSet::new(),
            observed_stopped: 0,
        })
    }

    pub fn request(&mut self, position: ChunkCoord) -> Result<LoadRequestState, LoadSubmitError> {
        if self.in_flight.contains(&position) {
            return Ok(LoadRequestState::Joined);
        }

        let worker = route_worker(position, self.workers.len());
        let Some(sender) = self.workers[worker].sender.as_ref() else {
            return Err(LoadSubmitError::Closed(position));
        };

        match sender.try_send(LoadCommand::Load(position)) {
            Ok(()) => {
                self.in_flight.insert(position);
                Ok(LoadRequestState::Queued)
            }
            Err(TrySendError::Full(LoadCommand::Load(position))) => {
                Err(LoadSubmitError::Backpressure(position))
            }
            Err(TrySendError::Disconnected(LoadCommand::Load(position))) => {
                Err(LoadSubmitError::Closed(position))
            }
        }
    }

    pub fn try_recv_completion(&mut self) -> Option<LoadCompletion> {
        loop {
            let receiver = self.completion_rx.as_ref()?;
            match receiver.try_recv() {
                Ok(WorkerEvent::Completion(completion)) => {
                    let completion = *completion;
                    self.in_flight.remove(&completion.position());
                    return Some(completion);
                }
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
    ) -> Result<Option<LoadCompletion>, RecvTimeoutError> {
        loop {
            let Some(receiver) = self.completion_rx.as_ref() else {
                return Ok(None);
            };
            match receiver.recv_timeout(timeout)? {
                WorkerEvent::Completion(completion) => {
                    let completion = *completion;
                    self.in_flight.remove(&completion.position());
                    return Ok(Some(completion));
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

    pub fn in_flight(&self) -> usize {
        self.in_flight.len()
    }

    pub fn shutdown(mut self) -> Vec<LoadCompletion> {
        self.shutdown_inner()
    }

    fn shutdown_inner(&mut self) -> Vec<LoadCompletion> {
        for worker in &mut self.workers {
            worker.sender.take();
        }

        let mut completions = Vec::new();
        if let Some(receiver) = self.completion_rx.take() {
            while self.observed_stopped < self.workers.len() {
                match receiver.recv() {
                    Ok(WorkerEvent::Completion(completion)) => {
                        let completion = *completion;
                        self.in_flight.remove(&completion.position());
                        completions.push(completion);
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

        self.in_flight.clear();
        completions
    }
}

impl Drop for AsyncLoadService {
    fn drop(&mut self) {
        let _ = self.shutdown_inner();
    }
}

fn validate_config(config: &AsyncLoadConfig) -> Result<(), AsyncLoadBuildError> {
    if config.workers == 0 || config.workers > MAX_ASYNC_LOAD_WORKERS {
        return Err(AsyncLoadBuildError::InvalidConfig(
            "workers must be in range 1..=32",
        ));
    }
    if config.queue_capacity == 0 || config.queue_capacity > MAX_ASYNC_LOAD_QUEUE_CAPACITY {
        return Err(AsyncLoadBuildError::InvalidConfig(
            "queue capacity must be in range 1..=65536",
        ));
    }
    if config.completion_capacity == 0
        || config.completion_capacity > MAX_ASYNC_LOAD_COMPLETION_CAPACITY
    {
        return Err(AsyncLoadBuildError::InvalidConfig(
            "completion capacity must be in range 1..=65536",
        ));
    }
    Ok(())
}

fn route_worker(position: ChunkCoord, workers: usize) -> usize {
    let region = RegionCoord::for_chunk(position);
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

fn load_worker_main(
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use cobblestone_core::{
        CHUNK_LIFECYCLE_GENERATED, CHUNK_LIFECYCLE_POPULATED, ChunkCoord, ChunkPatch, WorldStore,
    };

    use super::{AsyncLoadConfig, AsyncLoadService, LoadCompletion, LoadRequestState, region_path};
    use crate::{CompressionPolicy, RegionCoord, RegionFile};

    static TEMP_ID: AtomicU64 = AtomicU64::new(1);

    fn temp_root(name: &str) -> PathBuf {
        let id = TEMP_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "cobblestone-load-{name}-{}-{id}",
            std::process::id()
        ))
    }

    fn snapshot(position: ChunkCoord, state: u16) -> cobblestone_core::ChunkSnapshot {
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
