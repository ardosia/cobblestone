use std::num::NonZeroUsize;

use cobblestone_codec::CodecLimits;
use cobblestone_network::NetworkConfig;

/// Configuration for the native session host attached to one owning runtime.
#[derive(Debug, Clone)]
pub struct SessionHostConfig {
    pub(super) network: NetworkConfig,
    pub(super) limits: CodecLimits,
    pub(super) event_queue_capacity: NonZeroUsize,
    pub(super) session_command_capacity: NonZeroUsize,
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
