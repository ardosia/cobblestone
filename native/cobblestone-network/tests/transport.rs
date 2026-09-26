use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::time::Duration;

use bytes::Bytes;
use cobblestone_network::{NetworkConfig, NetworkServer};
use raknet_rust::client::{ClientSendOptions, RaknetClient, RaknetClientConfig};
use raknet_rust::low_level::protocol::Reliability as RaknetReliability;
use tokio::time::timeout;

fn allocate_loopback_addr() -> SocketAddr {
    let socket = std::net::UdpSocket::bind("127.0.0.1:0").expect("bind ephemeral UDP");
    socket.local_addr().expect("ephemeral address")
}

fn network_config(addr: SocketAddr) -> NetworkConfig {
    NetworkConfig::protocol8(
        addr,
        NonZeroUsize::new(32).expect("nonzero"),
        "cobblestone-network-test",
    )
}

#[tokio::test]
async fn fragmented_reliable_ordered_payload_reassembles() {
    let addr = allocate_loopback_addr();
    let mut server = NetworkServer::bind(network_config(addr))
        .await
        .expect("start server");
    let client_config = RaknetClientConfig {
        protocol_version: 8,
        ..RaknetClientConfig::default()
    };
    let mut client = RaknetClient::connect_with_config(addr, client_config)
        .await
        .expect("connect protocol8 client");
    let mut connection = timeout(Duration::from_secs(2), server.accept())
        .await
        .expect("accept timeout")
        .expect("accept connection");

    let payload = Bytes::from(vec![0x5a; 4096]);
    client
        .send_with_options(
            payload.clone(),
            ClientSendOptions {
                reliability: RaknetReliability::ReliableOrdered,
                ..ClientSendOptions::default()
            },
        )
        .await
        .expect("send fragmented payload");

    assert_eq!(
        timeout(Duration::from_secs(3), connection.recv())
            .await
            .expect("receive timeout")
            .expect("receive payload"),
        payload
    );

    client.disconnect(None).await.expect("disconnect client");
    server.shutdown().await.expect("shutdown server");
}
