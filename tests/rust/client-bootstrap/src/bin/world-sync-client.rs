use std::error::Error;
use std::net::SocketAddr;
use std::time::Duration;

use bytes::Bytes;
use cobblestone_codec::{
    BootstrapPacket, CodecError, CodecLimits, LoginPacket, RawPacket, decode_bootstrap_frame,
    decode_game_frame, encode_bootstrap_frame, encode_game_frame, packet_id,
};
use cobblestone_core::NativeBuffer;
use raknet_rust::client::{ClientSendOptions, RaknetClient, RaknetClientConfig, RaknetClientEvent};
use raknet_rust::low_level::protocol::Reliability;
use tokio::time::timeout;

const REQUEST_CHUNK_RADIUS_ID: u8 = 0x3d;
const UPDATE_BLOCK_ID: u8 = 0x13;

fn limits() -> CodecLimits {
    CodecLimits::new(
        2 * 1024 * 1024,
        2 * 1024 * 1024,
        2 * 1024 * 1024,
        4 * 1024 * 1024,
        2 * 1024 * 1024,
        256,
    )
}

fn raw_packets(payload: &[u8], limits: CodecLimits) -> Result<Vec<RawPacket>, CodecError> {
    let raw = decode_game_frame(payload, limits)?;
    if raw.id() != packet_id::BATCH {
        return Ok(vec![raw]);
    }

    let frame = encode_game_frame(&raw, limits)?;
    let BootstrapPacket::Batch(batch) = decode_bootstrap_frame(frame.as_slice(), limits)? else {
        return Ok(vec![raw]);
    };
    Ok(batch.packets().to_vec())
}

async fn send_frame(client: &mut RaknetClient, frame: &[u8]) -> Result<(), Box<dyn Error>> {
    client
        .send_with_options(
            Bytes::copy_from_slice(frame),
            ClientSendOptions {
                reliability: Reliability::ReliableOrdered,
                ..ClientSendOptions::default()
            },
        )
        .await?;
    Ok(())
}

async fn next_payload(client: &mut RaknetClient) -> Result<Bytes, Box<dyn Error>> {
    let payload = timeout(Duration::from_secs(5), async {
        loop {
            match client.next_event().await {
                Some(RaknetClientEvent::Packet { payload, .. }) => break Ok(payload),
                Some(RaknetClientEvent::Disconnected { reason }) => {
                    break Err(format!("client disconnected: {reason:?}"));
                }
                Some(_) => {}
                None => break Err("client event stream closed".to_owned()),
            }
        }
    })
    .await
    .map_err(|_| "client packet timeout")??;
    Ok(payload)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr: SocketAddr = std::env::args()
        .nth(1)
        .ok_or("missing server address")?
        .parse()?;
    let limits = limits();

    let mut client = RaknetClient::connect_with_config(
        addr,
        RaknetClientConfig {
            protocol_version: 8,
            ..RaknetClientConfig::default()
        },
    )
    .await?;

    let login = BootstrapPacket::Login(LoginPacket::protocol84(
        NativeBuffer::copy_from_slice(b"{}"),
        NativeBuffer::copy_from_slice(b"test-skin"),
    ));
    let frame = encode_bootstrap_frame(&login, limits)?;
    send_frame(&mut client, frame.as_slice()).await?;

    let mut saw_start_game = false;
    let mut saw_adventure = false;
    while !(saw_start_game && saw_adventure) {
        let payload = next_payload(&mut client).await?;
        for packet in raw_packets(&payload, limits)? {
            saw_start_game |= packet.id() == packet_id::START_GAME;
            saw_adventure |= packet.id() == packet_id::ADVENTURE_SETTINGS;
        }
    }

    let request = RawPacket::new(
        REQUEST_CHUNK_RADIUS_ID,
        NativeBuffer::copy_from_slice(&2_i32.to_be_bytes()),
    );
    let request_frame = encode_game_frame(&request, limits)?;
    send_frame(&mut client, request_frame.as_slice()).await?;

    let mut spawned = false;
    while !spawned {
        let payload = next_payload(&mut client).await?;
        for packet in raw_packets(&payload, limits)? {
            if packet.id() != packet_id::PLAY_STATUS {
                continue;
            }
            let frame = encode_game_frame(&packet, limits)?;
            if let BootstrapPacket::PlayStatus(status) =
                decode_bootstrap_frame(frame.as_slice(), limits)?
            {
                spawned |= status.status() == 3;
            }
        }
    }

    loop {
        let payload = next_payload(&mut client).await?;
        for packet in raw_packets(&payload, limits)? {
            if packet.id() != UPDATE_BLOCK_ID {
                continue;
            }
            let expected = [
                0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x80, 0x05, 0x01, 0xb0,
            ];
            if packet.body().as_slice() != expected {
                return Err(format!(
                    "unexpected UpdateBlock body: {:02x?}",
                    packet.body().as_slice()
                )
                .into());
            }

            println!("world-sync-client: update=verified packet=0x13 x=128 y=5 z=128 state=0x010");
            client.disconnect(None).await?;
            return Ok(());
        }
    }
}
