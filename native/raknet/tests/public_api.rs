// Adapted for Cobblestone from Ardosia network transport code under Apache-2.0.
// See docs/provenance/ARDOSIA_REUSE.md for exact source revisions and modifications.

use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::time::Duration;

use bytes::Bytes;
use cobblestone_raknet::{RaknetConfig, RaknetServer, Reliability};
use raknet_rust::client::{ClientSendOptions, RaknetClient, RaknetClientConfig, RaknetClientEvent};
use raknet_rust::low_level::protocol::Reliability as RaknetReliability;
use tokio::time::timeout;

fn allocate_loopback_addr() -> SocketAddr {
    let socket = std::net::UdpSocket::bind("127.0.0.1:0").expect("bind ephemeral UDP");
    socket.local_addr().expect("ephemeral address")
}

fn raknet_config(addr: SocketAddr) -> RaknetConfig {
    RaknetConfig::new(
        addr,
        NonZeroUsize::new(32).expect("nonzero"),
        "cobblestone-raknet-test",
    )
}

fn fixed_target_client_config() -> RaknetClientConfig {
    RaknetClientConfig {
        protocol_version: 8,
        ..RaknetClientConfig::default()
    }
}

#[tokio::test]
async fn fixed_target_roundtrips_reliable_ordered_payload() {
    let addr = allocate_loopback_addr();
    let mut server = RaknetServer::bind(raknet_config(addr))
        .await
        .expect("start fixed-target RakNet listener");

    let mut client = RaknetClient::connect_with_config(addr, fixed_target_client_config())
        .await
        .expect("connect fixed-target RakNet client");
    let mut connection = timeout(Duration::from_secs(2), server.accept())
        .await
        .expect("accept timeout")
        .expect("accepted connection");

    client
        .send_with_options(
            Bytes::from_static(b"client-to-server"),
            ClientSendOptions {
                reliability: RaknetReliability::ReliableOrdered,
                ..ClientSendOptions::default()
            },
        )
        .await
        .expect("send client payload");

    assert_eq!(
        timeout(Duration::from_secs(2), connection.recv())
            .await
            .expect("server receive timeout")
            .expect("server receive payload"),
        Bytes::from_static(b"client-to-server")
    );

    connection
        .send(
            Bytes::from_static(b"server-to-client"),
            Reliability::ReliableOrdered,
        )
        .await
        .expect("send server payload");

    let payload = timeout(Duration::from_secs(2), async {
        loop {
            match client.next_event().await {
                Some(RaknetClientEvent::Packet { payload, .. }) => break payload,
                Some(RaknetClientEvent::Disconnected { reason }) => {
                    panic!("client disconnected before reply: {reason:?}")
                }
                Some(_) => {}
                None => panic!("client event stream closed before reply"),
            }
        }
    })
    .await
    .expect("client receive timeout");

    assert_eq!(payload, Bytes::from_static(b"server-to-client"));

    client.disconnect(None).await.expect("disconnect client");
    server.shutdown().await.expect("shutdown server");
}
