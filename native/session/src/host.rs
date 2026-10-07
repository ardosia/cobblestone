mod runner;

use std::collections::HashMap;
use std::io;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex, MutexGuard, mpsc as std_mpsc};
use std::thread::{self, JoinHandle};

use cobblestone_raknet::RaknetConfig;
use cobblestone_wire::{CodecLimits, RawPacket};
use cobblestone_world::{ChunkCoord, WorldStore};
use crossbeam_channel::{Receiver, TryRecvError, bounded};
use tokio::sync::mpsc;

use crate::state::SessionState;
use crate::{
    ChunkWork, ChunkWorkCompletion, SessionDelivery, SessionId, SessionPacket,
    SessionWorldBootstrap, WorldViewSnapshot,
};
use runner::run_host;

pub(super) enum SessionCommand {
    Send {
        packet: SessionPacket,
        delivery: SessionDelivery,
    },
    SendMany {
        packets: Vec<(SessionPacket, SessionDelivery)>,
    },
    Disconnect,
}

#[derive(Clone)]
pub(super) struct SessionSlot {
    pub(super) commands: mpsc::Sender<SessionCommand>,
    pub(super) state: Arc<Mutex<SessionState>>,
}

/// Configuration for the native session host attached to one owning runtime.
#[derive(Debug, Clone)]
pub struct SessionHostConfig {
    raknet: RaknetConfig,
    limits: CodecLimits,
    event_queue_capacity: NonZeroUsize,
    session_command_capacity: NonZeroUsize,
    max_chunk_radius: i32,
}

impl SessionHostConfig {
    /// Creates an explicit bounded host configuration.
    #[must_use]
    pub const fn new(
        raknet: RaknetConfig,
        limits: CodecLimits,
        event_queue_capacity: NonZeroUsize,
        session_command_capacity: NonZeroUsize,
        max_chunk_radius: i32,
    ) -> Self {
        Self {
            raknet,
            limits,
            event_queue_capacity,
            session_command_capacity,
            max_chunk_radius,
        }
    }
}

/// Synchronous failures exposed by the native session host.
#[derive(Debug, thiserror::Error)]
pub enum SessionHostError {
    /// The dedicated native host thread could not be started.
    #[error("failed to spawn session host thread: {0}")]
    ThreadSpawn(#[source] io::Error),
    /// Tokio could not create the dedicated host runtime.
    #[error("failed to build session host runtime: {0}")]
    RuntimeBuild(#[source] io::Error),
    /// The listener/session mechanism failed before the host became ready.
    #[error("session host startup failed: {message}")]
    Startup {
        /// Failure text from the native session mechanism.
        message: String,
    },
    /// Startup ended without reporting success or failure.
    #[error("session host startup channel closed")]
    StartupClosed,
    /// No live session currently owns this process-local identity.
    #[error("unknown or stale session id {session_id}")]
    UnknownSession {
        /// Requested process-local identity.
        session_id: u64,
    },
    /// Session-owned fixed-target state rejected an operation.
    #[error("session {session_id} state error: {message}")]
    State {
        /// Requested process-local identity.
        session_id: u64,
        /// Stable mechanism error text.
        message: String,
    },
    /// The bounded command queue for this session is full.
    #[error("session {session_id} command queue is full")]
    CommandBackpressure {
        /// Requested process-local identity.
        session_id: u64,
    },
    /// The session task has already stopped accepting commands.
    #[error("session {session_id} command queue is closed")]
    CommandClosed {
        /// Requested process-local identity.
        session_id: u64,
    },
    /// The owner-side event receiver has disconnected.
    #[error("session host event channel is closed")]
    EventChannelClosed,
    /// The host shutdown signal could not be delivered.
    #[error("session host shutdown channel is closed")]
    ShutdownChannelClosed,
    /// The dedicated session host thread panicked.
    #[error("session host thread panicked")]
    ThreadPanicked,
}

/// Event delivered from the native session mechanism to its owning runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionHostEvent {
    /// A new fixed-target gameplay session was accepted.
    Connected {
        /// Stable process-local session identity.
        session_id: SessionId,
        /// Remote socket address rendered without exposing transport types.
        peer: String,
    },
    /// A validated fixed-target Login is waiting for world bootstrap values from the owner.
    LoginRequested {
        /// Stable process-local session identity.
        session_id: SessionId,
    },
    /// One post-bootstrap packet arrived after native session mechanics were applied.
    Packet {
        /// Stable process-local session identity.
        session_id: SessionId,
        /// Decoded wire packet.
        packet: SessionPacket,
    },
    /// A session ended or was closed by a bounded policy.
    Disconnected {
        /// Stable process-local session identity.
        session_id: SessionId,
        /// Stable human-readable mechanism reason.
        reason: String,
    },
}

/// Dedicated native session mechanism controlled synchronously by one owning runtime.
///
/// PHP/Zend code never runs on this host thread. The owner interacts only through bounded queues.
pub struct SessionHost {
    sessions: Arc<Mutex<HashMap<SessionId, SessionSlot>>>,
    limits: CodecLimits,
    events: Receiver<SessionHostEvent>,
    shutdown: mpsc::Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl SessionHost {
    /// Starts the dedicated native host thread and waits until the listener is bound.
    pub fn start(config: SessionHostConfig) -> Result<Self, SessionHostError> {
        let limits = config.limits;
        let (events_tx, events_rx) = bounded(config.event_queue_capacity.get());
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
        let (startup_tx, startup_rx) = std_mpsc::sync_channel(1);
        let sessions = Arc::new(Mutex::new(HashMap::new()));
        let thread_sessions = Arc::clone(&sessions);

        let thread = thread::Builder::new()
            .name("cobblestone-session-host".to_owned())
            .spawn(move || run_host(config, events_tx, thread_sessions, shutdown_rx, startup_tx))
            .map_err(SessionHostError::ThreadSpawn)?;

        match startup_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                sessions,
                limits,
                events: events_rx,
                shutdown: shutdown_tx,
                thread: Some(thread),
            }),
            Ok(Err(error)) => {
                let _ = thread.join();
                Err(error)
            }
            Err(_) => {
                let _ = thread.join();
                Err(SessionHostError::StartupClosed)
            }
        }
    }

    /// Polls one owner event without blocking the PHP/runtime thread.
    pub fn try_recv_event(&self) -> Result<Option<SessionHostEvent>, SessionHostError> {
        match self.events.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(SessionHostError::EventChannelClosed),
        }
    }

    /// Queues one packet for a live session without blocking on transport I/O.
    pub fn try_send(
        &self,
        session_id: SessionId,
        packet: SessionPacket,
        delivery: SessionDelivery,
    ) -> Result<(), SessionHostError> {
        self.try_command(session_id, SessionCommand::Send { packet, delivery })
    }

    /// Accepts one validated Login using semantic world values supplied by the owner runtime.
    pub fn accept_login(
        &self,
        session_id: SessionId,
        bootstrap: SessionWorldBootstrap,
    ) -> Result<(), SessionHostError> {
        let slot = self.slot(session_id)?;
        let mut state = lock_state(&slot.state);
        let packets = state
            .accept_login(bootstrap, self.limits)
            .map_err(|message| state_error(session_id, message))?;
        let command = SessionCommand::SendMany {
            packets: packets
                .into_iter()
                .map(|packet| (packet, SessionDelivery::ReliableOrdered))
                .collect(),
        };
        match slot.commands.try_send(command) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => {
                state.rollback_login_acceptance();
                Err(SessionHostError::CommandBackpressure {
                    session_id: session_id.get(),
                })
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                state.rollback_login_acceptance();
                Err(SessionHostError::CommandClosed {
                    session_id: session_id.get(),
                })
            }
        }
    }

    /// Returns the next session-owned chunk preparation snapshot after an optional session id.
    pub fn next_chunk_work(&self, after: Option<SessionId>) -> Option<ChunkWork> {
        let sessions = lock_sessions(&self.sessions);
        let mut ids = sessions.keys().copied().collect::<Vec<_>>();
        ids.sort_unstable();
        for session_id in ids {
            if after.is_some_and(|after| session_id <= after) {
                continue;
            }
            let Some(slot) = sessions.get(&session_id) else {
                continue;
            };
            if let Some(work) = lock_state(&slot.state).chunk_work(session_id) {
                return Some(work);
            }
        }
        None
    }

    /// Returns one live session's current chunk-work snapshot.
    pub fn chunk_work(&self, session_id: SessionId) -> Result<Option<ChunkWork>, SessionHostError> {
        let slot = self.slot(session_id)?;
        Ok(lock_state(&slot.state).chunk_work(session_id))
    }

    /// Pins one owner-prepared world chunk into the current pending session work item.
    pub fn mark_chunk_prepared(
        &self,
        session_id: SessionId,
        store: Arc<WorldStore>,
        position: ChunkCoord,
    ) -> Result<(), SessionHostError> {
        let slot = self.slot(session_id)?;
        lock_state(&slot.state)
            .mark_prepared_chunk(store, position)
            .map_err(|message| state_error(session_id, message))
    }

    /// Queues and commits one fully prepared initial view or post-spawn view transition.
    pub fn try_complete_chunk_work(
        &self,
        session_id: SessionId,
        store: &Arc<WorldStore>,
        chunks: Vec<RawPacket>,
    ) -> Result<ChunkWorkCompletion, SessionHostError> {
        let slot = self.slot(session_id)?;
        let mut state = lock_state(&slot.state);
        let plan = state
            .completion_plan(store, chunks, self.limits)
            .map_err(|message| state_error(session_id, message))?;
        let initial = plan.initial;
        let command = SessionCommand::SendMany {
            packets: plan
                .packets
                .into_iter()
                .map(|packet| (packet, SessionDelivery::ReliableOrdered))
                .collect(),
        };

        match slot.commands.try_send(command) {
            Ok(()) => {
                state
                    .commit_pending()
                    .map_err(|message| state_error(session_id, message))?;
                Ok(match initial {
                    Some(result) => ChunkWorkCompletion::Spawned(result),
                    None => ChunkWorkCompletion::Complete,
                })
            }
            Err(mpsc::error::TrySendError::Full(_)) => Ok(ChunkWorkCompletion::Backpressured),
            Err(mpsc::error::TrySendError::Closed(_)) => Ok(ChunkWorkCompletion::Gone),
        }
    }

    /// Returns active views backed by the supplied world store.
    pub fn world_view_snapshots(&self, store: &Arc<WorldStore>) -> Vec<WorldViewSnapshot> {
        let sessions = lock_sessions(&self.sessions);
        sessions
            .iter()
            .filter_map(|(&session_id, slot)| {
                let snapshot = lock_state(&slot.state).view_snapshot(session_id)?;
                Arc::ptr_eq(snapshot.store(), store).then_some(snapshot)
            })
            .collect()
    }

    /// Advances one active view's world-change cursor if it still belongs to the supplied store.
    pub fn update_view_cursor(
        &self,
        session_id: SessionId,
        store: &Arc<WorldStore>,
        cursor: u64,
    ) -> bool {
        let slot = {
            let sessions = lock_sessions(&self.sessions);
            sessions.get(&session_id).cloned()
        };
        let Some(slot) = slot else {
            return false;
        };
        lock_state(&slot.state).update_view_cursor(store, cursor)
    }

    /// Queues a disconnect for a live session.
    pub fn try_disconnect(&self, session_id: SessionId) -> Result<(), SessionHostError> {
        self.try_command(session_id, SessionCommand::Disconnect)
    }

    fn slot(&self, session_id: SessionId) -> Result<SessionSlot, SessionHostError> {
        lock_sessions(&self.sessions)
            .get(&session_id)
            .cloned()
            .ok_or(SessionHostError::UnknownSession {
                session_id: session_id.get(),
            })
    }

    fn try_command(
        &self,
        session_id: SessionId,
        command: SessionCommand,
    ) -> Result<(), SessionHostError> {
        let slot = self.slot(session_id)?;
        match slot.commands.try_send(command) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => Err(SessionHostError::CommandBackpressure {
                session_id: session_id.get(),
            }),
            Err(mpsc::error::TrySendError::Closed(_)) => Err(SessionHostError::CommandClosed {
                session_id: session_id.get(),
            }),
        }
    }

    /// Stops accepting sessions, closes live sessions, shuts down transport, and joins the host.
    pub fn shutdown(mut self) -> Result<(), SessionHostError> {
        match self.shutdown.try_send(()) {
            Ok(()) | Err(mpsc::error::TrySendError::Full(_)) => {}
            Err(mpsc::error::TrySendError::Closed(_)) => {
                return Err(SessionHostError::ShutdownChannelClosed);
            }
        }

        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| SessionHostError::ThreadPanicked)?;
        }
        Ok(())
    }
}

impl Drop for SessionHost {
    fn drop(&mut self) {
        let _ = self.shutdown.try_send(());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(super) fn lock_sessions(
    sessions: &Arc<Mutex<HashMap<SessionId, SessionSlot>>>,
) -> MutexGuard<'_, HashMap<SessionId, SessionSlot>> {
    match sessions.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub(super) fn lock_state(state: &Arc<Mutex<SessionState>>) -> MutexGuard<'_, SessionState> {
    match state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn state_error(session_id: SessionId, message: String) -> SessionHostError {
    SessionHostError::State {
        session_id: session_id.get(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddr};
    use std::num::NonZeroUsize;

    use cobblestone_raknet::RaknetConfig;
    use cobblestone_wire::CodecLimits;

    use super::SessionHostConfig;

    #[test]
    fn host_config_keeps_explicit_bounds() {
        let config = SessionHostConfig::new(
            RaknetConfig::new(
                SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 19132),
                NonZeroUsize::new(8).expect("nonzero"),
                "host-config-test",
            ),
            CodecLimits::new(4096, 4096, 4096, 4096, 1024, 16),
            NonZeroUsize::new(64).expect("nonzero"),
            NonZeroUsize::new(8).expect("nonzero"),
            3,
        );

        assert_eq!(config.event_queue_capacity.get(), 64);
        assert_eq!(config.session_command_capacity.get(), 8);
        assert_eq!(config.max_chunk_radius, 3);
    }
}
