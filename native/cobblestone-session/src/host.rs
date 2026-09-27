use std::collections::HashMap;
use std::io;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex, MutexGuard, mpsc as std_mpsc};
use std::thread::{self, JoinHandle};

use crossbeam_channel::{Receiver, Sender, TryRecvError, TrySendError, bounded};
use tokio::runtime::Builder;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tracing::{debug, warn};

use crate::{
    Session, SessionDelivery, SessionError, SessionId, SessionPacket, SessionServer,
};
use cobblestone_codec::CodecLimits;
use cobblestone_network::NetworkConfig;

/// Configuration for the native session host attached to one owning runtime.
#[derive(Debug, Clone)]
pub struct SessionHostConfig {
    network: NetworkConfig,
    limits: CodecLimits,
    event_queue_capacity: NonZeroUsize,
    session_command_capacity: NonZeroUsize,
}

impl SessionHostConfig {
    /// Creates an explicit bounded host configuration.
    #[must_use]
    pub const fn new(
        network: NetworkConfig,
        limits: CodecLimits,
        event_queue_capacity: NonZeroUsize,
        session_command_capacity: NonZeroUsize,
    ) -> Self {
        Self {
            network,
            limits,
            event_queue_capacity,
            session_command_capacity,
        }
    }
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
    /// One protocol-84 packet arrived after frame/Batch processing.
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

enum SessionCommand {
    Send {
        packet: SessionPacket,
        delivery: SessionDelivery,
    },
    Disconnect,
}

/// Dedicated native session mechanism controlled synchronously by one owning runtime.
///
/// PHP/Zend code never runs on this host thread. The owner interacts only through bounded queues:
/// nonblocking commands into live sessions and nonblocking event polling back on the owner thread.
pub struct SessionHost {
    sessions: Arc<Mutex<HashMap<SessionId, mpsc::Sender<SessionCommand>>>>,
    events: Receiver<SessionHostEvent>,
    shutdown: mpsc::Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl SessionHost {
    /// Starts the dedicated native host thread and waits until the listener is bound.
    pub fn start(config: SessionHostConfig) -> Result<Self, SessionHostError> {
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
        self.try_command(
            session_id,
            SessionCommand::Send { packet, delivery },
        )
    }

    /// Queues a disconnect for a live session.
    pub fn try_disconnect(&self, session_id: SessionId) -> Result<(), SessionHostError> {
        self.try_command(session_id, SessionCommand::Disconnect)
    }

    fn try_command(
        &self,
        session_id: SessionId,
        command: SessionCommand,
    ) -> Result<(), SessionHostError> {
        let sender = {
            let sessions = lock_sessions(&self.sessions);
            sessions
                .get(&session_id)
                .cloned()
                .ok_or(SessionHostError::UnknownSession {
                    session_id: session_id.get(),
                })?
        };

        match sender.try_send(command) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => {
                Err(SessionHostError::CommandBackpressure {
                    session_id: session_id.get(),
                })
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                Err(SessionHostError::CommandClosed {
                    session_id: session_id.get(),
                })
            }
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
            thread.join().map_err(|_| SessionHostError::ThreadPanicked)?;
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

fn lock_sessions(
    sessions: &Arc<Mutex<HashMap<SessionId, mpsc::Sender<SessionCommand>>>>,
) -> MutexGuard<'_, HashMap<SessionId, mpsc::Sender<SessionCommand>>> {
    match sessions.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn run_host(
    config: SessionHostConfig,
    events: Sender<SessionHostEvent>,
    sessions: Arc<Mutex<HashMap<SessionId, mpsc::Sender<SessionCommand>>>>,
    mut shutdown: mpsc::Receiver<()>,
    startup: std_mpsc::SyncSender<Result<(), SessionHostError>>,
) {
    let runtime = match Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = startup.send(Err(SessionHostError::RuntimeBuild(error)));
            return;
        }
    };

    let SessionHostConfig {
        network,
        limits,
        event_queue_capacity: _,
        session_command_capacity,
    } = config;

    runtime.block_on(async move {
        let mut server = match SessionServer::bind(network, limits).await {
            Ok(server) => server,
            Err(error) => {
                let _ = startup.send(Err(SessionHostError::Startup {
                    message: error.to_string(),
                }));
                return;
            }
        };

        if startup.send(Ok(())).is_err() {
            let _ = server.shutdown().await;
            return;
        }

        let mut tasks = JoinSet::new();

        loop {
            tokio::select! {
                _ = shutdown.recv() => break,
                accepted = server.accept() => {
                    match accepted {
                        Ok(session) => {
                            let id = session.id();
                            let peer = session.peer_addr().to_string();
                            let (commands_tx, commands_rx) =
                                mpsc::channel(session_command_capacity.get());

                            match events.try_send(SessionHostEvent::Connected {
                                session_id: id,
                                peer,
                            }) {
                                Ok(()) => {
                                    lock_sessions(&sessions).insert(id, commands_tx);
                                    tasks.spawn(run_session(
                                        session,
                                        commands_rx,
                                        events.clone(),
                                        Arc::clone(&sessions),
                                    ));
                                }
                                Err(TrySendError::Full(_)) => {
                                    warn!(
                                        session_id = id.get(),
                                        "owner event queue saturated while accepting session"
                                    );
                                    let _ = session.close().await;
                                }
                                Err(TrySendError::Disconnected(_)) => {
                                    let _ = session.close().await;
                                    break;
                                }
                            }
                        }
                        Err(error) => {
                            warn!(error = %error, "session listener failed");
                            break;
                        }
                    }
                }
                joined = tasks.join_next(), if !tasks.is_empty() => {
                    if let Some(Err(error)) = joined {
                        warn!(error = %error, "session task panicked");
                    }
                }
            }
        }

        let live = {
            let mut sessions = lock_sessions(&sessions);
            let live = sessions.values().cloned().collect::<Vec<_>>();
            sessions.clear();
            live
        };
        for commands in live {
            let _ = commands.try_send(SessionCommand::Disconnect);
        }

        if let Err(error) = server.shutdown().await {
            warn!(error = %error, "session server shutdown failed");
        }

        while let Some(joined) = tasks.join_next().await {
            if let Err(error) = joined {
                warn!(error = %error, "session task panicked during shutdown");
            }
        }

        debug!("session host stopped");
    });
}

async fn run_session(
    mut session: Session,
    mut commands: mpsc::Receiver<SessionCommand>,
    events: Sender<SessionHostEvent>,
    sessions: Arc<Mutex<HashMap<SessionId, mpsc::Sender<SessionCommand>>>>,
) {
    let id = session.id();
    let reason = loop {
        tokio::select! {
            command = commands.recv() => {
                match command {
                    Some(SessionCommand::Send { packet, delivery }) => {
                        if let Err(error) = session.send(&packet, delivery).await {
                            break format!("send failed: {error}");
                        }
                    }
                    Some(SessionCommand::Disconnect) => {
                        let _ = session.close().await;
                        break "owner disconnect".to_owned();
                    }
                    None => {
                        let _ = session.close().await;
                        break "owner command channel closed".to_owned();
                    }
                }
            }
            received = session.recv() => {
                match received {
                    Ok(packet) => {
                        match events.try_send(SessionHostEvent::Packet {
                            session_id: id,
                            packet,
                        }) {
                            Ok(()) => {}
                            Err(TrySendError::Full(_)) => {
                                let _ = session.close().await;
                                break "owner event queue backpressure".to_owned();
                            }
                            Err(TrySendError::Disconnected(_)) => {
                                let _ = session.close().await;
                                break "owner event channel closed".to_owned();
                            }
                        }
                    }
                    Err(error) => break session_end_reason(&error),
                }
            }
        }
    };

    lock_sessions(&sessions).remove(&id);
    let _ = events.try_send(SessionHostEvent::Disconnected {
        session_id: id,
        reason,
    });
}

fn session_end_reason(error: &SessionError) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddr};
    use std::num::NonZeroUsize;

    use cobblestone_codec::CodecLimits;
    use cobblestone_network::NetworkConfig;

    use super::SessionHostConfig;

    #[test]
    fn host_config_keeps_explicit_bounds() {
        let config = SessionHostConfig::new(
            NetworkConfig::protocol8(
                SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 19132),
                NonZeroUsize::new(8).expect("nonzero"),
                "host-config-test",
            ),
            CodecLimits::new(4096, 4096, 4096, 4096, 1024, 16),
            NonZeroUsize::new(64).expect("nonzero"),
            NonZeroUsize::new(8).expect("nonzero"),
        );

        assert_eq!(config.event_queue_capacity.get(), 64);
        assert_eq!(config.session_command_capacity.get(), 8);
    }
}
