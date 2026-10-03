use std::collections::HashMap;
use std::sync::{Arc, Mutex, mpsc as std_mpsc};

use crossbeam_channel::{Sender, TrySendError};
use tokio::runtime::Builder;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tracing::{debug, warn};

use super::{SessionCommand, SessionHostConfig, SessionHostError, SessionHostEvent, lock_sessions};
use crate::{Session, SessionError, SessionId, SessionServer};

pub(super) fn run_host(
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
