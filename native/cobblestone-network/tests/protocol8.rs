use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::time::Duration;

use cobblestone_network::{NetworkConfig, NetworkServer};
use raknet_rust::client::{RaknetClient, RaknetClientConfig};
use tokio::net::UdpSocket;
use tokio::time::timeout;

const REQUEST1: u8 = 0x05;
const REPLY1: u8 = 0x06;
const INCOMPATIBLE: u8 = 0x19;
const MAGIC: [u8; 16] = [
    0x00, 0xff, 0xff, 0x00, 0xfe, 0xfe, 0xfe, 0xfe, 0xfd, 0xfd, 0xfd, 0xfd, 0x12, 0x34, 0x56, 0x78,
];

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

fn raw_request1(protocol: u8, mtu: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(mtu);
    out.push(REQUEST1);
    out.extend_from_slice(&MAGIC);
    out.push(protocol);
    out.resize(mtu, 0);
    out
}

async fn reply_id(addr: SocketAddr, protocol: u8) -> u8 {
    let socket = UdpSocket::bind("127.0.0.1:0").await.expect("bind UDP");
    socket
        .send_to(&raw_request1(protocol, 1200), addr)
        .await
        .expect("send request1");

    let mut buffer = [0u8; 2048];
    let (len, _) = timeout(Duration::from_secs(2), socket.recv_from(&mut buffer))
        .await
        .expect("request1 reply timeout")
        .expect("receive request1 reply");
    assert!(len > 0);
    buffer[0]
}

#[tokio::test]
async fn raw_protocol8_request1_is_accepted() {
    let addr = allocate_loopback_addr();
    let server = NetworkServer::bind(network_config(addr))
        .await
        .expect("start server");

    assert_eq!(reply_id(addr, 8).await, REPLY1);

    server.shutdown().await.expect("shutdown server");
}

#[tokio::test]
async fn protocol11_request1_is_rejected() {
    let addr = allocate_loopback_addr();
    let server = NetworkServer::bind(network_config(addr))
        .await
        .expect("start server");

    assert_eq!(reply_id(addr, 11).await, INCOMPATIBLE);

    server.shutdown().await.expect("shutdown server");
}

#[tokio::test]
async fn protocol8_transport_client_reaches_accept() {
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

    let connection = timeout(Duration::from_secs(2), server.accept())
        .await
        .expect("accept timeout")
        .expect("accepted connection");
    assert_eq!(connection.peer_addr().ip(), addr.ip());

    client.disconnect(None).await.expect("disconnect client");
    server.shutdown().await.expect("shutdown server");
}
