// Adapted for Cobblestone from Ardosia network transport code under Apache-2.0.
// See docs/provenance/ARDOSIA_REUSE.md for exact source revisions and modifications.

use std::collections::HashMap;

use bytes::Bytes;
use raknet_rust::server::{PeerId, RaknetServer, RaknetServerEvent, SendOptions};
use tokio::sync::{mpsc, oneshot, watch};

use crate::connection::Connection;
use crate::{NetworkError, Reliability};

pub(crate) const COMMAND_QUEUE_CAPACITY: usize = 4096;
pub(crate) const PER_CONNECTION_INBOUND_CAPACITY: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CloseState {
    Open,
    Closed,
    Backpressure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AcceptDispatch {
    Enqueued,
    Full,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InboundDispatch {
    Enqueued,
    Full,
    Closed,
}

pub(crate) enum BackendCommand {
    Send {
        peer_id: PeerId,
        payload: Bytes,
        reliability: Reliability,
        response: oneshot::Sender<Result<(), NetworkError>>,
    },
    Disconnect {
        peer_id: PeerId,
        response: oneshot::Sender<Result<(), NetworkError>>,
    },
    Shutdown {
        response: oneshot::Sender<Result<(), NetworkError>>,
    },
}

struct PeerState {
    inbound: mpsc::Sender<Bytes>,
    close: watch::Sender<CloseState>,
}

pub(crate) async fn run_backend(
    mut server: RaknetServer,
    mut commands: mpsc::Receiver<BackendCommand>,
    accept_tx: mpsc::Sender<Result<Connection, NetworkError>>,
    command_tx: mpsc::Sender<BackendCommand>,
) {
    let mut peers: HashMap<PeerId, PeerState> = HashMap::new();
    let mut shutdown_response = None;

    'run: loop {
        tokio::select! {
            command = commands.recv() => {
                match command {
                    Some(BackendCommand::Send {
                        peer_id,
                        payload,
                        reliability,
                        response,
                    }) => {
                        let result = server
                            .send_with_options(
                                peer_id,
                                payload,
                                SendOptions {
                                    reliability: reliability.into_transport(),
                                    ..SendOptions::default()
                                },
                            )
                            .await
                            .map_err(NetworkError::from);
                        let _ = response.send(result);
                    }
                    Some(BackendCommand::Disconnect { peer_id, response }) => {
                        let result = server.disconnect(peer_id).await.map_err(NetworkError::from);
                        let _ = response.send(result);
                    }
                    Some(BackendCommand::Shutdown { response }) => {
                        shutdown_response = Some(response);
                        break 'run;
                    }
                    None => break 'run,
                }
            }
            event = server.next_event() => {
                let Some(event) = event else {
                    break 'run;
                };

                if handle_server_event(
                    &mut server,
                    event,
                    &mut peers,
                    &accept_tx,
                    &command_tx,
                )
                .await
                {
                    break 'run;
                }
            }
        }
    }

    for peer in peers.into_values() {
        let _ = peer.close.send(CloseState::Closed);
    }

    let shutdown_result = server.shutdown().await.map_err(NetworkError::from);
    if let Some(response) = shutdown_response {
        let _ = response.send(shutdown_result);
    }
}

async fn handle_server_event(
    server: &mut RaknetServer,
    event: RaknetServerEvent,
    peers: &mut HashMap<PeerId, PeerState>,
    accept_tx: &mpsc::Sender<Result<Connection, NetworkError>>,
    command_tx: &mpsc::Sender<BackendCommand>,
) -> bool {
    match event {
        RaknetServerEvent::PeerConnected { peer_id, addr, .. } => {
            let (inbound_tx, inbound_rx) = mpsc::channel(PER_CONNECTION_INBOUND_CAPACITY);
            let (close_tx, close_rx) = watch::channel(CloseState::Open);

            peers.insert(
                peer_id,
                PeerState {
                    inbound: inbound_tx,
                    close: close_tx,
                },
            );

            let connection =
                Connection::new(peer_id, addr, inbound_rx, close_rx, command_tx.clone());

            match publish_connection(accept_tx, connection) {
                AcceptDispatch::Enqueued => false,
                AcceptDispatch::Full => {
                    close_peer_for_backpressure(server, peers, peer_id).await;
                    false
                }
                AcceptDispatch::Closed => {
                    if let Some(peer) = peers.remove(&peer_id) {
                        let _ = peer.close.send(CloseState::Closed);
                    }
                    let _ = server.disconnect(peer_id).await;
                    true
                }
            }
        }
        RaknetServerEvent::Packet {
            peer_id, payload, ..
        } => {
            let dispatch = match peers.get(&peer_id) {
                Some(peer) => dispatch_inbound(peer, payload),
                None => return false,
            };

            match dispatch {
                InboundDispatch::Enqueued => {}
                InboundDispatch::Full => {
                    close_peer_for_backpressure(server, peers, peer_id).await;
                }
                InboundDispatch::Closed => {
                    if let Some(peer) = peers.remove(&peer_id) {
                        let _ = peer.close.send(CloseState::Closed);
                    }
                    let _ = server.disconnect(peer_id).await;
                }
            }
            false
        }
        RaknetServerEvent::PeerDisconnected { peer_id, .. } => {
            if let Some(peer) = peers.remove(&peer_id) {
                let _ = peer.close.send(CloseState::Closed);
            }
            false
        }
        RaknetServerEvent::DecodeError { .. } => false,
        RaknetServerEvent::WorkerError { shard_id, message } => {
            let message = format!("RakNet worker {shard_id} failed: {message}");
            let _ = accept_tx.try_send(Err(NetworkError::BackendFailure { message }));
            true
        }
        RaknetServerEvent::WorkerStopped { shard_id } => {
            let message = format!("RakNet worker {shard_id} stopped unexpectedly");
            let _ = accept_tx.try_send(Err(NetworkError::BackendFailure { message }));
            true
        }
        RaknetServerEvent::Metrics { .. } => false,
        _ => false,
    }
}

fn publish_connection(
    accept_tx: &mpsc::Sender<Result<Connection, NetworkError>>,
    connection: Connection,
) -> AcceptDispatch {
    match accept_tx.try_send(Ok(connection)) {
        Ok(()) => AcceptDispatch::Enqueued,
        Err(mpsc::error::TrySendError::Full(_)) => AcceptDispatch::Full,
        Err(mpsc::error::TrySendError::Closed(_)) => AcceptDispatch::Closed,
    }
}

fn dispatch_inbound(peer: &PeerState, payload: Bytes) -> InboundDispatch {
    match peer.inbound.try_send(payload) {
        Ok(()) => InboundDispatch::Enqueued,
        Err(mpsc::error::TrySendError::Full(_)) => InboundDispatch::Full,
        Err(mpsc::error::TrySendError::Closed(_)) => InboundDispatch::Closed,
    }
}

async fn close_peer_for_backpressure(
    server: &mut RaknetServer,
    peers: &mut HashMap<PeerId, PeerState>,
    peer_id: PeerId,
) {
    if let Some(peer) = peers.remove(&peer_id) {
        let _ = peer.close.send(CloseState::Backpressure);
    }
    let _ = server.disconnect(peer_id).await;
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use tokio::sync::{mpsc, watch};

    use std::net::{Ipv4Addr, SocketAddr};

    use raknet_rust::server::PeerId;

    use super::{
        AcceptDispatch, CloseState, InboundDispatch, PeerState, dispatch_inbound,
        publish_connection,
    };
    use crate::connection::Connection;

    fn connection(peer_id: u64) -> Connection {
        let (_inbound_tx, inbound_rx) = mpsc::channel(1);
        let (_close_tx, close_rx) = watch::channel(CloseState::Open);
        let (command_tx, _command_rx) = mpsc::channel(1);

        Connection::new(
            PeerId::from_u64(peer_id),
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 19132),
            inbound_rx,
            close_rx,
            command_tx,
        )
    }

    #[test]
    fn bounded_accept_dispatch_reports_full_without_growing() {
        let (accept_tx, mut accept_rx) = mpsc::channel(1);

        assert_eq!(
            publish_connection(&accept_tx, connection(1)),
            AcceptDispatch::Enqueued
        );
        assert_eq!(
            publish_connection(&accept_tx, connection(2)),
            AcceptDispatch::Full
        );
        assert!(accept_rx.try_recv().is_ok());
        assert!(accept_rx.try_recv().is_err());
    }

    #[test]
    fn accept_dispatch_reports_closed_receiver() {
        let (accept_tx, accept_rx) = mpsc::channel(1);
        drop(accept_rx);

        assert_eq!(
            publish_connection(&accept_tx, connection(1)),
            AcceptDispatch::Closed
        );
    }

    #[test]
    fn bounded_inbound_dispatch_reports_full_without_growing() {
        let (inbound, mut receiver) = mpsc::channel(1);
        let (close, _close_rx) = watch::channel(CloseState::Open);
        let peer = PeerState { inbound, close };

        assert_eq!(
            dispatch_inbound(&peer, Bytes::from_static(b"first")),
            InboundDispatch::Enqueued
        );
        assert_eq!(
            dispatch_inbound(&peer, Bytes::from_static(b"second")),
            InboundDispatch::Full
        );
        assert_eq!(
            receiver.try_recv().expect("first payload remains queued"),
            Bytes::from_static(b"first")
        );
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn inbound_dispatch_reports_closed_receiver() {
        let (inbound, receiver) = mpsc::channel(1);
        let (close, _close_rx) = watch::channel(CloseState::Open);
        let peer = PeerState { inbound, close };
        drop(receiver);

        assert_eq!(
            dispatch_inbound(&peer, Bytes::from_static(b"payload")),
            InboundDispatch::Closed
        );
    }
}
