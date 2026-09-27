use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::time::Duration;

use bytes::Bytes;
use cobblestone_codec::{CodecLimits, RawPacket, encode_game_frame};
use cobblestone_core::NativeBuffer;
use cobblestone_network::NetworkConfig;
use cobblestone_session::{SessionDelivery, SessionError, SessionPacket, SessionServer};
use raknet_rust::client::{ClientSendOptions, RaknetClient, RaknetClientConfig, RaknetClientEvent};
use raknet_rust::low_level::protocol::Reliability as RaknetReliability;
use tokio::time::timeout;

fn allocate_loopback_addr() -> SocketAddr {
    let socket = std::net::UdpSocket::bind("127.0.0.1:0").expect("bind ephemeral UDP");
    socket.local_addr().expect("ephemeral address")
}

fn limits() -> CodecLimits {
    CodecLimits::new(4096, 4096, 4096, 4096, 2048, 32)
}

fn network_config(addr: SocketAddr) -> NetworkConfig {
    NetworkConfig::protocol8(
        addr,
        NonZeroUsize::new(8).expect("nonzero"),
        "MCPE;Cobblestone Session Test;84;;0;8",
    )
}

fn client_config() -> RaknetClientConfig {
    RaknetClientConfig {
        protocol_version: 8,
        ..RaknetClientConfig::default()
    }
}

async fn next_client_packet(client: &mut RaknetClient) -> Bytes {
    timeout(Duration::from_secs(2), async {
        loop {
            match client.next_event().await {
                Some(RaknetClientEvent::Packet { payload, .. }) => break payload,
                Some(RaknetClientEvent::Disconnected { reason }) => {
                    panic!("client disconnected before packet: {reason:?}")
                }
                Some(_) => {}
                None => panic!("client event stream closed before packet"),
            }
        }
    })
    .await
    .expect("client packet timeout")
}

#[tokio::test]
async fn session_round_trips_protocol84_without_exposing_raknet_connection() {
    let addr = allocate_loopback_addr();
    let mut server = SessionServer::bind(network_config(addr), limits())
        .await
        .expect("start session server");
    let mut client = RaknetClient::connect_with_config(addr, client_config())
        .await
        .expect("connect protocol8 client");
    let mut session = timeout(Duration::from_secs(2), server.accept())
        .await
        .expect("session accept timeout")
        .expect("accepted session");

    assert_eq!(session.id().get(), 1);
    assert_eq!(session.peer_addr().ip(), addr.ip());

    let inbound = RawPacket::new(0x10, NativeBuffer::from_vec(vec![1, 2, 3]));
    let inbound_frame = encode_game_frame(&inbound, limits()).expect("encode inbound frame");
    client
        .send_with_options(
            Bytes::copy_from_slice(inbound_frame.as_slice()),
            ClientSendOptions {
                reliability: RaknetReliability::ReliableOrdered,
                ..ClientSendOptions::default()
            },
        )
        .await
        .expect("send client packet");

    let received = timeout(Duration::from_secs(2), session.recv())
        .await
        .expect("session receive timeout")
        .expect("session packet");
    assert_eq!(received.id(), 0x10);
    assert_eq!(received.body().as_slice(), &[1, 2, 3]);

    let outbound = SessionPacket::new(0x20, NativeBuffer::from_vec(vec![4, 5, 6]));
    session
        .send(&outbound, SessionDelivery::ReliableOrdered)
        .await
        .expect("send session packet");

    let outbound_frame = next_client_packet(&mut client).await;
    let decoded = cobblestone_codec::decode_game_frame(outbound_frame.as_ref(), limits())
        .expect("decode server frame");
    assert_eq!(decoded.id(), 0x20);
    assert_eq!(decoded.body().as_slice(), &[4, 5, 6]);

    session.close().await.expect("close session");
    server.shutdown().await.expect("shutdown session server");
}

#[tokio::test]
async fn malformed_protocol84_input_closes_at_session_boundary() {
    let addr = allocate_loopback_addr();
    let mut server = SessionServer::bind(network_config(addr), limits())
        .await
        .expect("start session server");
    let mut client = RaknetClient::connect_with_config(addr, client_config())
        .await
        .expect("connect protocol8 client");
    let mut session = timeout(Duration::from_secs(2), server.accept())
        .await
        .expect("session accept timeout")
        .expect("accepted session");

    client
        .send_with_options(
            Bytes::from_static(b"not-a-game-frame"),
            ClientSendOptions {
                reliability: RaknetReliability::ReliableOrdered,
                ..ClientSendOptions::default()
            },
        )
        .await
        .expect("send malformed payload");

    let error = timeout(Duration::from_secs(2), session.recv())
        .await
        .expect("session receive timeout")
        .expect_err("malformed payload must fail");
    assert!(matches!(error, SessionError::MalformedInput { .. }));

    server.shutdown().await.expect("shutdown session server");
}

#[tokio::test]
async fn outbound_codec_limits_fail_before_transport_submission() {
    let addr = allocate_loopback_addr();
    let tight_limits = CodecLimits::new(8, 4096, 4096, 4096, 2048, 32);
    let mut server = SessionServer::bind(network_config(addr), tight_limits)
        .await
        .expect("start session server");
    let _client = RaknetClient::connect_with_config(addr, client_config())
        .await
        .expect("connect protocol8 client");
    let session = timeout(Duration::from_secs(2), server.accept())
        .await
        .expect("session accept timeout")
        .expect("accepted session");

    let packet = SessionPacket::new(0x10, NativeBuffer::from_vec(vec![0; 32]));
    let error = session
        .send(&packet, SessionDelivery::ReliableOrdered)
        .await
        .expect_err("over-limit frame must fail");
    assert!(matches!(error, SessionError::OutboundCodec { .. }));

    session.close().await.expect("close session");
    server.shutdown().await.expect("shutdown session server");
}
