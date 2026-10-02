// Adapted for Cobblestone from Ardosia network transport code under Apache-2.0.
// See docs/provenance/ARDOSIA_REUSE.md for exact source revisions and modifications.

use std::net::SocketAddr;

use bytes::Bytes;
use raknet_rust::server::PeerId;
use tokio::sync::{mpsc, oneshot, watch};

use crate::backend::{BackendCommand, CloseState};
use crate::{NetworkError, Reliability};

/// One accepted RakNet connection carrying opaque connected payloads.
pub struct Connection {
    peer_id: PeerId,
    peer_addr: SocketAddr,
    inbound: mpsc::Receiver<Bytes>,
    close: watch::Receiver<CloseState>,
    commands: mpsc::Sender<BackendCommand>,
}

impl Connection {
    pub(crate) fn new(
        peer_id: PeerId,
        peer_addr: SocketAddr,
        inbound: mpsc::Receiver<Bytes>,
        close: watch::Receiver<CloseState>,
        commands: mpsc::Sender<BackendCommand>,
    ) -> Self {
        Self {
            peer_id,
            peer_addr,
            inbound,
            close,
            commands,
        }
    }

    /// Returns the remote socket address associated with this connection.
    #[must_use]
    pub const fn peer_addr(&self) -> SocketAddr {
        self.peer_addr
    }

    /// Receives the next opaque connected payload from the peer.
    pub async fn recv(&mut self) -> Result<Bytes, NetworkError> {
        loop {
            if let Some(error) = close_state_error(*self.close.borrow()) {
                return Err(error);
            }

            tokio::select! {
                changed = self.close.changed() => {
                    if changed.is_err() {
                        return Err(NetworkError::BackendStopped);
                    }
                    if let Some(error) = close_state_error(*self.close.borrow()) {
                        return Err(error);
                    }
                }
                payload = self.inbound.recv() => {
                    if let Some(payload) = payload {
                        return Ok(payload);
                    }
                    return match *self.close.borrow() {
                        CloseState::Backpressure => Err(NetworkError::Backpressure),
                        CloseState::Closed => Err(NetworkError::ConnectionClosed),
                        CloseState::Open => Err(NetworkError::BackendStopped),
                    };
                }
            }
        }
    }

    /// Sends one opaque connected payload with the requested RakNet delivery semantics.
    pub async fn send(&self, payload: Bytes, reliability: Reliability) -> Result<(), NetworkError> {
        if let Some(error) = close_state_error(*self.close.borrow()) {
            return Err(error);
        }

        let (response_tx, response_rx) = oneshot::channel();
        try_send_command(
            &self.commands,
            BackendCommand::Send {
                peer_id: self.peer_id,
                payload,
                reliability,
                response: response_tx,
            },
        )?;

        response_rx
            .await
            .map_err(|_| NetworkError::BackendStopped)?
    }

    /// Requests a transport-level disconnect for this peer.
    pub async fn close(&self) -> Result<(), NetworkError> {
        if let Some(error) = close_state_error(*self.close.borrow()) {
            return Err(error);
        }

        let (response_tx, response_rx) = oneshot::channel();
        try_send_command(
            &self.commands,
            BackendCommand::Disconnect {
                peer_id: self.peer_id,
                response: response_tx,
            },
        )?;

        response_rx
            .await
            .map_err(|_| NetworkError::BackendStopped)?
    }
}

fn try_send_command(
    commands: &mpsc::Sender<BackendCommand>,
    command: BackendCommand,
) -> Result<(), NetworkError> {
    match commands.try_send(command) {
        Ok(()) => Ok(()),
        Err(mpsc::error::TrySendError::Full(_)) => Err(NetworkError::CommandBackpressure),
        Err(mpsc::error::TrySendError::Closed(_)) => Err(NetworkError::BackendStopped),
    }
}

fn close_state_error(state: CloseState) -> Option<NetworkError> {
    match state {
        CloseState::Open => None,
        CloseState::Closed => Some(NetworkError::ConnectionClosed),
        CloseState::Backpressure => Some(NetworkError::Backpressure),
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use raknet_rust::server::PeerId;
    use tokio::sync::{mpsc, oneshot};

    use super::try_send_command;
    use crate::backend::BackendCommand;
    use crate::{NetworkError, Reliability};

    fn command(peer_id: PeerId) -> BackendCommand {
        let (response, _receiver) = oneshot::channel();
        BackendCommand::Send {
            peer_id,
            payload: Bytes::from_static(b"payload"),
            reliability: Reliability::Reliable,
            response,
        }
    }

    #[test]
    fn full_command_queue_reports_backpressure() {
        let (sender, _receiver) = mpsc::channel(1);
        try_send_command(&sender, command(PeerId::from_u64(1))).expect("first command should fit");

        assert!(matches!(
            try_send_command(&sender, command(PeerId::from_u64(2))),
            Err(NetworkError::CommandBackpressure)
        ));
    }

    #[test]
    fn closed_command_queue_reports_backend_stopped() {
        let (sender, receiver) = mpsc::channel(1);
        drop(receiver);

        assert!(matches!(
            try_send_command(&sender, command(PeerId::from_u64(1))),
            Err(NetworkError::BackendStopped)
        ));
    }
}
