// Adapted for Cobblestone from Ardosia network transport code under Apache-2.0.
// See docs/provenance/ARDOSIA_REUSE.md for exact source revisions and modifications.

use std::net::SocketAddr;
use std::num::NonZeroUsize;

use raknet_rust::low_level::transport::TransportConfig;

/// RakNet protocol version used by the initial Cobblestone compatibility target.
pub const RAKNET_PROTOCOL: u8 = 8;

/// Errors produced while translating a NetworkConfig.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NetworkConfigError {
    /// The pinned RakNet transport rejected the translated configuration.
    #[error("RakNet rejected the transport configuration: {message}")]
    TransportRejected {
        /// Validation message returned by the transport.
        message: String,
    },
}

/// Fixed-target transport configuration.
///
/// Cobblestone's initial compatibility target is RakNet protocol 8 with the legacy cookie-less
/// handshake. Protocol selection is intentionally not exposed here until an accepted change
/// expands the compatibility target.
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    bind_addr: SocketAddr,
    max_connections: NonZeroUsize,
    advertisement: String,
    worker_shards: Option<NonZeroUsize>,
}

impl NetworkConfig {
    /// Creates a protocol-8 listener configuration.
    ///
    /// The advertisement is opaque transport payload and is forwarded unchanged. Game-level
    /// construction of the advertisement belongs above this crate.
    #[must_use]
    pub fn protocol8(
        bind_addr: SocketAddr,
        max_connections: NonZeroUsize,
        advertisement: impl Into<String>,
    ) -> Self {
        Self {
            bind_addr,
            max_connections,
            advertisement: advertisement.into(),
            worker_shards: None,
        }
    }

    /// Overrides the transport's automatic worker-shard count.
    #[must_use]
    pub fn with_worker_shards(mut self, worker_shards: NonZeroUsize) -> Self {
        self.worker_shards = Some(worker_shards);
        self
    }

    pub(crate) const fn max_connections(&self) -> NonZeroUsize {
        self.max_connections
    }

    pub(crate) const fn worker_shards(&self) -> Option<NonZeroUsize> {
        self.worker_shards
    }

    pub(crate) fn to_transport_config(&self) -> Result<TransportConfig, NetworkConfigError> {
        let transport = TransportConfig {
            bind_addr: self.bind_addr,
            supported_protocols: vec![RAKNET_PROTOCOL],
            max_sessions: self.max_connections.get(),
            advertisement: self.advertisement.clone(),
            send_cookie: false,
            ..TransportConfig::default()
        };

        transport
            .validate()
            .map_err(|error| NetworkConfigError::TransportRejected {
                message: error.to_string(),
            })?;

        Ok(transport)
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddr};
    use std::num::NonZeroUsize;

    use super::{NetworkConfig, RAKNET_PROTOCOL};

    #[test]
    fn fixed_target_maps_to_protocol8_cookie_less_transport() {
        let config = NetworkConfig::protocol8(
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 19132),
            NonZeroUsize::new(20).expect("nonzero"),
            "cobblestone-network-test",
        );
        let transport = config
            .to_transport_config()
            .expect("valid transport config");

        assert_eq!(RAKNET_PROTOCOL, 8);
        assert_eq!(transport.supported_protocols, vec![8]);
        assert!(!transport.send_cookie);
        assert!(transport.allow_legacy_request2_fallback);
        assert!(!transport.reject_ambiguous_request2);
        assert_eq!(transport.advertisement, "cobblestone-network-test");
    }

    #[test]
    fn explicit_worker_shards_are_retained() {
        let config = NetworkConfig::protocol8(
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 19132),
            NonZeroUsize::new(20).expect("nonzero"),
            "cobblestone-network-test",
        )
        .with_worker_shards(NonZeroUsize::new(4).expect("nonzero"));

        assert_eq!(config.worker_shards().map(NonZeroUsize::get), Some(4));
    }
}
