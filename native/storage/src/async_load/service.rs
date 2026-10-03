use std::collections::HashSet;
use std::sync::mpsc::{
    Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError, sync_channel,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cobblestone_world::ChunkCoord;

use super::{
    AsyncLoadBuildError, AsyncLoadConfig, LoadCompletion, LoadRequestState, LoadSubmitError,
    MAX_ASYNC_LOAD_COMPLETION_CAPACITY, MAX_ASYNC_LOAD_QUEUE_CAPACITY, MAX_ASYNC_LOAD_WORKERS,
};
use crate::async_io::route_chunk_worker;

use super::worker;

pub(super) enum LoadCommand {
    Load(ChunkCoord),
}

pub(super) enum WorkerEvent {
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
                    worker::load_worker_main(root, world_uuid, command_rx, completion_tx);
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

        let worker = route_chunk_worker(position, self.workers.len());
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
