mod command;
mod config;
mod error;
mod event;
mod runner;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, mpsc as std_mpsc};
use std::thread::{self, JoinHandle};

use crossbeam_channel::{Receiver, TryRecvError, bounded};
use tokio::sync::mpsc;

use crate::{SessionDelivery, SessionId, SessionPacket};
use command::SessionCommand;
use runner::run_host;

pub use config::SessionHostConfig;
pub use error::SessionHostError;
pub use event::SessionHostEvent;

/// Dedicated native session mechanism controlled synchronously by one owning runtime.
///
/// PHP/Zend code never runs on this host thread. The owner interacts only through bounded queues.
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
        self.try_command(session_id, SessionCommand::Send { packet, delivery })
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

pub(super) fn lock_sessions(
    sessions: &Arc<Mutex<HashMap<SessionId, mpsc::Sender<SessionCommand>>>>,
) -> MutexGuard<'_, HashMap<SessionId, mpsc::Sender<SessionCommand>>> {
    match sessions.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
