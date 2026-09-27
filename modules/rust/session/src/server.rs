use std::num::NonZeroU64;

use cobblestone_codec::CodecLimits;
use cobblestone_network::{NetworkConfig, NetworkServer};
use tracing::debug;

use crate::{Session, SessionError, SessionId};

/// Fixed-target session listener.
///
/// The owning runtime sees stable session IDs and decoded packets. RakNet lifecycle, bounded
/// transport queues, game-frame markers, and Batch decompression remain below this API.
pub struct SessionServer {
    network: NetworkServer,
    limits: CodecLimits,
    next_id: u64,
}

impl SessionServer {
    /// Binds a RakNet-8 listener and prepares fixed-target session decoding.
    pub async fn bind(network: NetworkConfig, limits: CodecLimits) -> Result<Self, SessionError> {
        Ok(Self {
            network: NetworkServer::bind(network).await?,
            limits,
            next_id: 1,
        })
    }

    /// Waits for and returns the next accepted session.
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

        Ok(Session::new(id, connection, self.limits))
    }

    /// Gracefully shuts down the underlying listener/backend.
    pub async fn shutdown(self) -> Result<(), SessionError> {
        self.network.shutdown().await?;
        Ok(())
    }
}
