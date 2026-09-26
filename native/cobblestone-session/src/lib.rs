#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Production protocol-84 session mechanism above Cobblestone's RakNet transport and wire codec.
//!
//! This crate hides RakNet connection objects and Batch/compression framing from the owning
//! runtime. It does not own gameplay semantics, players, worlds, plugins, or PHP/Zend state.

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::num::NonZeroU64;

use bytes::Bytes;
use cobblestone_codec::{
    BootstrapPacket, CodecError, CodecLimits, RawPacket, decode_bootstrap_frame, decode_game_frame,
    encode_game_frame, packet_id,
};
use cobblestone_core::NativeBuffer;
use cobblestone_network::{Connection, NetworkConfig, NetworkError, NetworkServer, Reliability};
use thiserror::Error;
use tracing::{debug, warn};

/// Stable identity assigned to one accepted gameplay session.
///
/// Session IDs are process-local and never intentionally reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(NonZeroU64);

impl SessionId {
    /// Returns the nonzero integer representation.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// One decoded protocol-84 packet delivered to the owning runtime.
///
/// The outer game marker and Batch/compression envelope are already removed. Packet bodies remain
/// wire data and must not be exposed as ordinary gameplay/plugin APIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionPacket {
    id: u8,
    body: NativeBuffer,
}

impl SessionPacket {
    /// Creates one packet from its protocol-84 packet ID and body bytes.
    #[must_use]
    pub fn new(id: u8, body: NativeBuffer) -> Self {
        Self { id, body }
    }

    /// Returns the protocol-84 packet ID.
    #[must_use]
    pub const fn id(&self) -> u8 {
        self.id
    }

    /// Returns the bytes after the packet ID.
    #[must_use]
    pub const fn body(&self) -> &NativeBuffer {
        &self.body
    }

    fn from_raw(raw: &RawPacket) -> Self {
        Self {
            id: raw.id(),
            body: raw.body().clone(),
        }
    }

    fn to_raw(&self) -> RawPacket {
        RawPacket::new(self.id, self.body.clone())
    }
}

/// Delivery semantics requested by the session layer.
///
/// This mirrors the transport choices without exposing the RakNet dependency itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionDelivery {
    /// Best-effort delivery without retransmission or ordering.
    Unreliable,
    /// Best-effort sequenced delivery where newer messages supersede older ones.
    UnreliableSequenced,
    /// Retransmit until acknowledged without ordering.
    Reliable,
    /// Retransmit and deliver in order.
    ReliableOrdered,
    /// Retransmit with sequenced supersession.
    ReliableSequenced,
}

impl SessionDelivery {
    const fn reliability(self) -> Reliability {
        match self {
            Self::Unreliable => Reliability::Unreliable,
            Self::UnreliableSequenced => Reliability::UnreliableSequenced,
            Self::Reliable => Reliability::Reliable,
            Self::ReliableOrdered => Reliability::ReliableOrdered,
            Self::ReliableSequenced => Reliability::ReliableSequenced,
        }
    }
}

/// Failures surfaced by the production session mechanism.
#[derive(Debug, Error)]
pub enum SessionError {
    /// The underlying bounded RakNet transport rejected or lost the operation.
    #[error(transparent)]
    Network(#[from] NetworkError),

    /// An inbound connected payload was not valid within the configured protocol-84 limits.
    ///
    /// The session closes the peer before returning this error.
    #[error("malformed protocol-84 session input: {source}")]
    MalformedInput {
        /// Codec failure that made the inbound payload invalid.
        #[source]
        source: CodecError,
    },

    /// An outbound packet could not be encoded within the configured protocol-84 limits.
    #[error("invalid outbound protocol-84 session packet: {source}")]
    OutboundCodec {
        /// Codec failure that prevented transmission.
        #[source]
        source: CodecError,
    },

    /// The process-local nonzero session identity space was exhausted.
    #[error("session identity space exhausted")]
    IdExhausted,
}

/// Fixed-target session listener.
///
/// The owning runtime sees stable session IDs and decoded protocol-84 packets. RakNet lifecycle,
/// bounded transport queues, game-frame markers, and Batch decompression remain below this API.
pub struct SessionServer {
    network: NetworkServer,
    limits: CodecLimits,
    next_id: u64,
}

impl SessionServer {
    /// Binds a RakNet-8 listener and prepares protocol-84 session decoding.
    pub async fn bind(network: NetworkConfig, limits: CodecLimits) -> Result<Self, SessionError> {
        Ok(Self {
            network: NetworkServer::bind(network).await?,
            limits,
            next_id: 1,
        })
    }

    /// Waits for and returns the next accepted session.
    ///
    /// Accepted sessions inherit the transport's bounded per-connection inbound queue and bounded
    /// backend command queue. Backpressure therefore remains typed and finite rather than growing
    /// an application backlog.
    pub async fn accept(&mut self) -> Result<Session, SessionError> {
        let connection = self.network.accept().await?;
        let id = match NonZeroU64::new(self.next_id) {
            Some(id) => SessionId(id),
            None => {
                let _ = connection.close().await;
                return Err(SessionError::IdExhausted);
            }
        };
        self.next_id = self.next_id.checked_add(1).unwrap_or_default();

        debug!(session_id = id.get(), peer = %connection.peer_addr(), "session accepted");

        Ok(Session {
            id,
            peer_addr: connection.peer_addr(),
            connection,
            limits: self.limits,
            pending: VecDeque::new(),
        })
    }

    /// Gracefully shuts down the underlying listener/backend.
    pub async fn shutdown(self) -> Result<(), SessionError> {
        self.network.shutdown().await?;
        Ok(())
    }
}

/// One accepted protocol-84 session owned by a single runtime.
pub struct Session {
    id: SessionId,
    peer_addr: SocketAddr,
    connection: Connection,
    limits: CodecLimits,
    pending: VecDeque<SessionPacket>,
}

impl Session {
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

    /// Receives the next protocol-84 packet.
    ///
    /// Batch packets are decompressed and flattened internally, so callers receive one inner
    /// packet at a time. Malformed or over-limit input closes the peer before the error is
    /// returned. Transport backpressure/disconnect failures remain typed network errors.
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
                    "closing malformed protocol-84 session"
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

    /// Sends one protocol-84 packet using the requested delivery semantics.
    ///
    /// Encoding occurs before the bounded transport command is submitted, so an over-limit frame
    /// is rejected without consuming transport queue capacity.
    pub async fn send(
        &self,
        packet: &SessionPacket,
        delivery: SessionDelivery,
    ) -> Result<(), SessionError> {
        let frame = encode_game_frame(&packet.to_raw(), self.limits)
            .map_err(|source| SessionError::OutboundCodec { source })?;
        self.connection
            .send(
                Bytes::copy_from_slice(frame.as_slice()),
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

fn decode_connected_payload(
    payload: &[u8],
    limits: CodecLimits,
) -> Result<VecDeque<SessionPacket>, CodecError> {
    let raw = decode_game_frame(payload, limits)?;
    if raw.id() != packet_id::BATCH {
        return Ok(VecDeque::from([SessionPacket::from_raw(&raw)]));
    }

    let BootstrapPacket::Batch(batch) = decode_bootstrap_frame(payload, limits)? else {
        return Err(CodecError::UnsupportedPacket { id: raw.id() });
    };

    if batch.packets().is_empty() {
        return Err(CodecError::EmptyPacket);
    }

    Ok(batch
        .packets()
        .iter()
        .map(SessionPacket::from_raw)
        .collect())
}

#[cfg(test)]
mod tests {
    use cobblestone_codec::{BatchPacket, CodecLimits, RawPacket, encode_bootstrap_frame};
    use cobblestone_core::NativeBuffer;

    use super::{SessionPacket, decode_connected_payload};

    fn limits() -> CodecLimits {
        CodecLimits::new(4096, 4096, 4096, 4096, 1024, 16)
    }

    #[test]
    fn batch_is_flattened_before_reaching_owner() {
        let batch = cobblestone_codec::BootstrapPacket::Batch(BatchPacket::new(vec![
            RawPacket::new(0x10, NativeBuffer::from_vec(vec![1, 2])),
            RawPacket::new(0x20, NativeBuffer::from_vec(vec![3, 4, 5])),
        ]));
        let frame = encode_bootstrap_frame(&batch, limits()).expect("encode batch");

        let packets = decode_connected_payload(frame.as_slice(), limits()).expect("decode batch");
        assert_eq!(
            packets.into_iter().collect::<Vec<_>>(),
            vec![
                SessionPacket::new(0x10, NativeBuffer::from_vec(vec![1, 2])),
                SessionPacket::new(0x20, NativeBuffer::from_vec(vec![3, 4, 5])),
            ]
        );
    }
}
