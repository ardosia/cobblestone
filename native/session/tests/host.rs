use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use cobblestone_raknet::RaknetConfig;
use cobblestone_session::{
    ChunkWorkCompletion, SessionHost, SessionHostConfig, SessionHostEvent, SessionWorldBootstrap,
};
use cobblestone_wire::{
    BootstrapPacket, CodecLimits, DimensionId, FULL_CHUNK_DATA_ID, LoginPacket, RawPacket,
    encode_bootstrap_packet, encode_game_frame, packet_id,
};
use cobblestone_world::WorldStore;
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
        RaknetConfig::new(
            addr,
            NonZeroUsize::new(8).expect("nonzero"),
            "MCPE;Cobblestone Host Test;84;;0;8",
        ),
        limits(),
        NonZeroUsize::new(32).expect("nonzero"),
        NonZeroUsize::new(8).expect("nonzero"),
        3,
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

async fn send_packet(client: &mut RaknetClient, packet: RawPacket) {
    let frame = encode_game_frame(&packet, limits()).expect("encode client frame");
    client
        .send_with_options(
            Bytes::copy_from_slice(frame.as_ref()),
            ClientSendOptions {
                reliability: RaknetReliability::ReliableOrdered,
                ..ClientSendOptions::default()
            },
        )
        .await
        .expect("send client packet");
}

#[tokio::test]
async fn host_owns_join_and_view_state_before_forwarding_gameplay_packets() {
    let addr = allocate_loopback_addr();
    let host = SessionHost::start(host_config(addr)).expect("start session host");
    let mut client = RaknetClient::connect_with_config(addr, client_config())
        .await
        .expect("connect fixed-target RakNet client");

    let session_id = match next_host_event(&host).await {
        SessionHostEvent::Connected {
            session_id,
            peer: _,
        } => session_id,
        event => panic!("expected connected event, got {event:?}"),
    };

    let login = encode_bootstrap_packet(
        &BootstrapPacket::Login(LoginPacket::new(
            Bytes::copy_from_slice(b"{}"),
            Bytes::copy_from_slice(b"test-skin"),
        )),
        limits(),
    )
    .expect("encode login");
    send_packet(&mut client, login).await;
    assert!(matches!(
        next_host_event(&host).await,
        SessionHostEvent::LoginRequested { session_id: observed } if observed == session_id
    ));

    host.accept_login(
        session_id,
        SessionWorldBootstrap::new(
            1234,
            DimensionId::Overworld,
            1,
            [0, 64, 0],
            0,
            true,
            "Host Test".to_owned(),
        )
        .expect("world bootstrap"),
    )
    .expect("accept login");

    send_packet(
        &mut client,
        RawPacket::new(
            packet_id::REQUEST_CHUNK_RADIUS,
            Bytes::copy_from_slice(&1_i32.to_be_bytes()),
        ),
    )
    .await;

    let work = timeout(Duration::from_secs(2), async {
        loop {
            if let Some(work) = host.chunk_work(session_id).expect("chunk work") {
                break work;
            }
            sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("chunk-work timeout");
    assert_eq!(work.positions().len(), 9);
    assert_eq!(work.send_positions().len(), 9);

    let store = Arc::new(WorldStore::new());
    for &position in work.positions() {
        store.ensure_chunk(position, 1).expect("resident chunk");
        host.mark_chunk_prepared(session_id, Arc::clone(&store), position)
            .expect("mark prepared chunk");
    }
    let packets = work
        .send_positions()
        .iter()
        .map(|_| RawPacket::new(FULL_CHUNK_DATA_ID, Bytes::new()))
        .collect();
    assert!(matches!(
        host.try_complete_chunk_work(session_id, &store, packets)
            .expect("complete initial view"),
        ChunkWorkCompletion::Spawned(_)
    ));

    let gameplay = RawPacket::new(0x20, Bytes::from_static(b"owner-boundary"));
    send_packet(&mut client, gameplay).await;
    match next_host_event(&host).await {
        SessionHostEvent::Packet {
            session_id: observed,
            packet,
        } => {
            assert_eq!(observed, session_id);
            assert_eq!(packet.id(), 0x20);
            assert_eq!(packet.body().as_ref(), b"owner-boundary");
        }
        event => panic!("expected gameplay packet event, got {event:?}"),
    }

    host.try_disconnect(session_id)
        .expect("queue owner disconnect");
    assert!(matches!(
        next_host_event(&host).await,
        SessionHostEvent::Disconnected { session_id: observed, .. } if observed == session_id
    ));

    host.shutdown().expect("shutdown session host");
}

#[tokio::test]
async fn host_rejects_gameplay_before_login() {
    let addr = allocate_loopback_addr();
    let host = SessionHost::start(host_config(addr)).expect("start session host");
    let mut client = RaknetClient::connect_with_config(addr, client_config())
        .await
        .expect("connect fixed-target RakNet client");
    let session_id = match next_host_event(&host).await {
        SessionHostEvent::Connected { session_id, .. } => session_id,
        event => panic!("expected connected event, got {event:?}"),
    };

    send_packet(
        &mut client,
        RawPacket::new(0x20, Bytes::from_static(b"too-early")),
    )
    .await;
    match next_host_event(&host).await {
        SessionHostEvent::Disconnected {
            session_id: observed,
            reason,
        } => {
            assert_eq!(observed, session_id);
            assert!(reason.contains("expected Login"));
        }
        event => panic!("expected disconnect, got {event:?}"),
    }

    // Drain/observe the client disconnect event so the test does not leave a live transport task.
    let _ = timeout(Duration::from_secs(2), async {
        while let Some(event) = client.next_event().await {
            if matches!(event, RaknetClientEvent::Disconnected { .. }) {
                break;
            }
        }
    })
    .await;
    host.shutdown().expect("shutdown session host");
}
