use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::time::Duration;

use bytes::Bytes;
use cobblestone_core::NativeBuffer;
use cobblestone_protocol84::{CodecLimits, RawPacket, decode_game_frame, encode_game_frame};
use cobblestone_session::{
    SessionDelivery, SessionHost, SessionHostConfig, SessionHostEvent, SessionPacket,
};
use cobblestone_transport::NetworkConfig;
use raknet_rust::client::{ClientSendOptions, RaknetClient, RaknetClientConfig, RaknetClientEvent};
use raknet_rust::low_level::protocol::Reliability as RaknetReliability;
use tokio::time::{sleep, timeout};

fn allocate_loopback_addr() -> SocketAddr {
    let socket = std::net::UdpSocket::bind("127.0.0.1:0").expect("bind ephemeral UDP");
    socket.local_addr().expect("ephemeral address")
}

fn limits() -> CodecLimits {
    CodecLimits::new(4096, 4096, 4096, 4096, 2048, 32)
}

fn host_config(addr: SocketAddr) -> SessionHostConfig {
    SessionHostConfig::new(
        NetworkConfig::protocol8(
            addr,
            NonZeroUsize::new(8).expect("nonzero"),
            "MCPE;Cobblestone Host Test;84;;0;8",
        ),
        limits(),
        NonZeroUsize::new(32).expect("nonzero"),
        NonZeroUsize::new(8).expect("nonzero"),
    )
}

fn client_config() -> RaknetClientConfig {
    RaknetClientConfig {
        protocol_version: 8,
        ..RaknetClientConfig::default()
    }
}

async fn next_host_event(host: &SessionHost) -> SessionHostEvent {
    timeout(Duration::from_secs(2), async {
        loop {
            if let Some(event) = host.try_recv_event().expect("poll host event") {
                break event;
            }
            sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("host event timeout")
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
async fn host_routes_packets_without_zend_or_raknet_objects_at_owner_boundary() {
    let addr = allocate_loopback_addr();
    let host = SessionHost::start(host_config(addr)).expect("start session host");
    let mut client = RaknetClient::connect_with_config(addr, client_config())
        .await
        .expect("connect protocol8 client");

    let session_id = match next_host_event(&host).await {
        SessionHostEvent::Connected {
            session_id,
            peer: _,
        } => session_id,
        event => panic!("expected connected event, got {event:?}"),
    };

    let inbound = RawPacket::new(0x10, NativeBuffer::from_vec(vec![1, 2, 3]));
    let frame = encode_game_frame(&inbound, limits()).expect("encode inbound frame");
    client
        .send_with_options(
            Bytes::copy_from_slice(frame.as_slice()),
            ClientSendOptions {
                reliability: RaknetReliability::ReliableOrdered,
                ..ClientSendOptions::default()
            },
        )
        .await
        .expect("send client packet");

    match next_host_event(&host).await {
        SessionHostEvent::Packet {
            session_id: observed,
            packet,
        } => {
            assert_eq!(observed, session_id);
            assert_eq!(packet.id(), 0x10);
            assert_eq!(packet.body().as_slice(), &[1, 2, 3]);
        }
        event => panic!("expected packet event, got {event:?}"),
    }

    host.try_send(
        session_id,
        SessionPacket::new(0x20, NativeBuffer::from_vec(vec![4, 5, 6])),
        SessionDelivery::ReliableOrdered,
    )
    .expect("queue owner packet");

    let outbound = next_client_packet(&mut client).await;
    let decoded = decode_game_frame(outbound.as_ref(), limits()).expect("decode host frame");
    assert_eq!(decoded.id(), 0x20);
    assert_eq!(decoded.body().as_slice(), &[4, 5, 6]);

    host.try_disconnect(session_id)
        .expect("queue owner disconnect");

    match next_host_event(&host).await {
        SessionHostEvent::Disconnected {
            session_id: observed,
            reason,
        } => {
            assert_eq!(observed, session_id);
            assert!(reason.contains("owner disconnect"));
        }
        event => panic!("expected disconnected event, got {event:?}"),
    }

    host.shutdown().expect("shutdown session host");
}
