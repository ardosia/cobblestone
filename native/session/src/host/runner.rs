use std::collections::HashMap;
use std::sync::{Arc, Mutex, mpsc as std_mpsc};

use cobblestone_wire::CodecLimits;
use crossbeam_channel::{Sender, TrySendError};
use tokio::runtime::Builder;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tracing::{debug, warn};

use super::{
    SessionCommand, SessionHostConfig, SessionHostError, SessionHostEvent, SessionSlot,
    lock_sessions, lock_state,
};
use crate::state::{InboundAction, SessionState};
use crate::{Session, SessionError, SessionId, SessionServer};

pub(super) fn run_host(
    config: SessionHostConfig,
    events: Sender<SessionHostEvent>,
    sessions: Arc<Mutex<HashMap<SessionId, SessionSlot>>>,
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
        raknet,
        limits,
        event_queue_capacity: _,
        session_command_capacity,
        max_chunk_radius,
    } = config;

    runtime.block_on(async move {
        let mut server = match SessionServer::bind(raknet, limits).await {
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
                            let state = Arc::new(Mutex::new(SessionState::new(max_chunk_radius)));

                            match events.try_send(SessionHostEvent::Connected {
                                session_id: id,
                                peer,
                            }) {
                                Ok(()) => {
                                    lock_sessions(&sessions).insert(
                                        id,
                                        SessionSlot {
                                            commands: commands_tx,
                                            state: Arc::clone(&state),
                                        },
                                    );
                                    tasks.spawn(run_session(
                                        session,
                                        commands_rx,
                                        events.clone(),
                                        Arc::clone(&sessions),
                                        state,
                                        limits,
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
            let live = sessions
                .values()
                .map(|slot| slot.commands.clone())
                .collect::<Vec<_>>();
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
    sessions: Arc<Mutex<HashMap<SessionId, SessionSlot>>>,
    state: Arc<Mutex<SessionState>>,
    limits: CodecLimits,
) {
    let id = session.id();
    let reason = loop {
        let deferred = { lock_state(&state).take_deferred() };
        if let Some(packet) = deferred {
            match process_inbound(&state, id, packet, limits, &events) {
                Ok(()) => continue,
                Err(reason) => {
                    let _ = session.close().await;
                    break reason;
                }
            }
        }

        tokio::select! {
            command = commands.recv() => {
                match command {
                    Some(SessionCommand::Send { packet, delivery }) => {
                        if let Err(error) = session.send(&packet, delivery).await {
                            break format!("send failed: {error}");
                        }
                    }
                    Some(SessionCommand::SendMany { packets }) => {
                        let mut failed = None;
                        for (packet, delivery) in packets {
                            if let Err(error) = session.send(&packet, delivery).await {
                                failed = Some(format!("send failed: {error}"));
                                break;
                            }
                        }
                        if let Some(reason) = failed {
                            break reason;
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
                        if let Err(reason) = process_inbound(&state, id, packet, limits, &events) {
                            let _ = session.close().await;
                            break reason;
                        }
                    }
                    Err(error) => break session_end_reason(&error),
                }
            }
        }
    };

    lock_sessions(&sessions).remove(&id);
    // Release active and temporary chunk pins before the owner observes disconnect completion.
    drop(state);
    let _ = events.try_send(SessionHostEvent::Disconnected {
        session_id: id,
        reason,
    });
}

fn process_inbound(
    state: &Arc<Mutex<SessionState>>,
    session_id: SessionId,
    packet: crate::SessionPacket,
    limits: CodecLimits,
    events: &Sender<SessionHostEvent>,
) -> Result<(), String> {
    let action = lock_state(state)
        .handle_packet(packet, limits)
        .map_err(|error| format!("fixed-target session state rejected packet: {error}"))?;
    let event = match action {
        InboundAction::LoginRequested => Some(SessionHostEvent::LoginRequested { session_id }),
        InboundAction::Forward(packet) => Some(SessionHostEvent::Packet { session_id, packet }),
        InboundAction::Consumed => None,
    };

    let Some(event) = event else {
        return Ok(());
    };
    match events.try_send(event) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(_)) => Err("owner event queue backpressure".into()),
        Err(TrySendError::Disconnected(_)) => Err("owner event channel closed".into()),
    }
}

fn session_end_reason(error: &SessionError) -> String {
    error.to_string()
}
