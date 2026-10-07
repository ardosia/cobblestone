use std::collections::VecDeque;
use std::net::SocketAddr;

use bytes::Bytes;
use cobblestone_raknet::Connection;
use cobblestone_wire::{CodecError, CodecLimits, encode_game_frame};
use tracing::warn;

use crate::wire::decode_connected_payload;
use crate::{SessionDelivery, SessionError, SessionId, SessionPacket};

/// One accepted fixed-target session owned by a single runtime.
pub struct Session {
    id: SessionId,
    peer_addr: SocketAddr,
    connection: Connection,
    limits: CodecLimits,
    pending: VecDeque<SessionPacket>,
}

impl Session {
    pub(crate) fn new(id: SessionId, connection: Connection, limits: CodecLimits) -> Self {
        Self {
            id,
            peer_addr: connection.peer_addr(),
            connection,
            limits,
            pending: VecDeque::new(),
        }
    }

    /// Returns this process-local stable session identity.
    #[must_use]
    pub const fn id(&self) -> SessionId {
        self.id
    }

    /// Returns the remote socket address captured when the session was accepted.
    #[must_use]
    pub const fn peer_addr(&self) -> SocketAddr {
        self.peer_addr
    }

    /// Receives the next fixed-target packet.
    pub async fn recv(&mut self) -> Result<SessionPacket, SessionError> {
        if let Some(packet) = self.pending.pop_front() {
            return Ok(packet);
        }

        let payload = self.connection.recv().await?;
        let packets = match decode_connected_payload(&payload, self.limits) {
            Ok(packets) => packets,
            Err(source) => {
                warn!(
                    session_id = self.id.get(),
                    peer = %self.peer_addr,
                    error = %source,
                    "closing malformed session"
                );
                let _ = self.connection.close().await;
                return Err(SessionError::MalformedInput { source });
            }
        };

        self.pending.extend(packets);
        self.pending
            .pop_front()
            .ok_or(SessionError::MalformedInput {
                source: CodecError::EmptyPacket,
            })
    }

    /// Sends one packet using the requested delivery semantics.
    pub async fn send(
        &self,
        packet: &SessionPacket,
        delivery: SessionDelivery,
    ) -> Result<(), SessionError> {
        let frame = encode_game_frame(&packet.to_raw(), self.limits)
            .map_err(|source| SessionError::OutboundCodec { source })?;
        self.connection
            .send(
                Bytes::copy_from_slice(frame.as_ref()),
                delivery.reliability(),
            )
            .await?;
        Ok(())
    }

    /// Requests a transport-level disconnect for this session.
    pub async fn close(&self) -> Result<(), SessionError> {
        self.connection.close().await?;
        Ok(())
    }
}
