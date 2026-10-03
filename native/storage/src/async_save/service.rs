use std::collections::VecDeque;
use std::sync::mpsc::{
    Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError, sync_channel,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cobblestone_world::ChunkSnapshot;

use super::{
    AsyncSaveBuildError, AsyncSaveConfig, CompactionCompletion, CompactionSubmitError,
    MAX_ASYNC_SAVE_COMPLETION_CAPACITY, MAX_ASYNC_SAVE_QUEUE_CAPACITY, MAX_ASYNC_SAVE_WORKERS,
    SaveCompletion, SaveSubmitError,
};
use crate::{
    RegionCoord,
    async_io::{route_chunk_worker, route_region_worker},
};

use super::worker;

pub(super) enum SaveCommand {
    Save(ChunkSnapshot),
    Compact(RegionCoord),
}

pub(super) enum WorkerEvent {
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
                    worker::save_worker_main(
                        root,
                        world_uuid,
                        compression,
                        command_rx,
                        completion_tx,
                    );
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
        let worker = route_chunk_worker(snapshot.position(), self.workers.len());
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
