use std::error::Error;
use std::io::Write;
use std::net::SocketAddr;
use std::time::Duration;

use bytes::Bytes;
use cobblestone_protocol84::{
    BootstrapPacket, CodecError, CodecLimits, DimensionId, LoginPacket, RawPacket,
    decode_bootstrap_frame, decode_game_frame, encode_bootstrap_frame, encode_game_frame,
    packet_id,
};
use cobblestone_runtime::NativeBuffer;
use raknet_rust::client::{ClientSendOptions, RaknetClient, RaknetClientConfig, RaknetClientEvent};
use raknet_rust::low_level::protocol::Reliability;
use tokio::time::timeout;

const REQUEST_CHUNK_RADIUS_ID: u8 = 0x3d;
const CHUNK_RADIUS_UPDATED_ID: u8 = 0x3e;
const UPDATE_BLOCK_ID: u8 = 0x13;

fn move_player_body(position: [f32; 3]) -> NativeBuffer {
    let mut body = Vec::with_capacity(34);
    body.extend_from_slice(&0_i64.to_be_bytes());
    for value in [position[0], position[1], position[2], 0.0, 0.0, 0.0] {
        body.extend_from_slice(&value.to_bits().to_be_bytes());
    }
    body.push(0);
    body.push(1);
    NativeBuffer::from_vec(body)
}

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

async fn send_radius_request(
    client: &mut RaknetClient,
    limits: CodecLimits,
    radius: i32,
) -> Result<(), Box<dyn Error>> {
    let request = RawPacket::new(
        REQUEST_CHUNK_RADIUS_ID,
        NativeBuffer::copy_from_slice(&radius.to_be_bytes()),
    );
    let frame = encode_game_frame(&request, limits)?;
    send_frame(client, frame.as_slice()).await
}

async fn verify_radius_cycle(
    client: &mut RaknetClient,
    limits: CodecLimits,
) -> Result<(), Box<dyn Error>> {
    send_radius_request(client, limits, 3).await?;

    let mut saw_radius_three = false;
    let mut entering = std::collections::BTreeSet::new();
    while !saw_radius_three || entering.len() < 24 {
        let payload = next_payload(client)
            .await
            .map_err(|error| -> Box<dyn Error> {
                format!("waiting for radius grow: {error}").into()
            })?;
        for packet in raw_packets(&payload, limits)? {
            if packet.id() == CHUNK_RADIUS_UPDATED_ID {
                if packet.body().as_slice() == 3_i32.to_be_bytes() {
                    saw_radius_three = true;
                }
                continue;
            }
            if packet.id() != cobblestone_protocol84::FULL_CHUNK_DATA_ID || packet.body().len() < 8
            {
                continue;
            }

            let body = packet.body().as_slice();
            let chunk_x = i32::from_be_bytes(body[0..4].try_into()?);
            let chunk_z = i32::from_be_bytes(body[4..8].try_into()?);
            let in_outer_square = (5..=11).contains(&chunk_x) && (5..=11).contains(&chunk_z);
            let in_old_square = (6..=10).contains(&chunk_x) && (6..=10).contains(&chunk_z);
            if in_outer_square && !in_old_square {
                entering.insert((chunk_x, chunk_z));
            }
        }
    }

    if entering.len() != 24 {
        return Err(format!("unexpected radius-grow chunk set: {entering:?}").into());
    }

    send_radius_request(client, limits, 1).await?;
    loop {
        let payload = next_payload(client)
            .await
            .map_err(|error| -> Box<dyn Error> {
                format!("waiting for radius shrink: {error}").into()
            })?;
        let mut saw_radius_one = false;
        for packet in raw_packets(&payload, limits)? {
            if packet.id() == CHUNK_RADIUS_UPDATED_ID
                && packet.body().as_slice() == 1_i32.to_be_bytes()
            {
                saw_radius_one = true;
            }
        }
        if saw_radius_one {
            break;
        }
    }

    Ok(())
}

async fn send_movement(
    client: &mut RaknetClient,
    limits: CodecLimits,
    position: [f32; 3],
) -> Result<(), Box<dyn Error>> {
    let movement = RawPacket::new(packet_id::MOVE_PLAYER, move_player_body(position));
    let movement_frame = encode_game_frame(&movement, limits)?;
    client
        .send_with_options(
            Bytes::copy_from_slice(movement_frame.as_slice()),
            ClientSendOptions {
                reliability: Reliability::UnreliableSequenced,
                ..ClientSendOptions::default()
            },
        )
        .await?;
    Ok(())
}

async fn wait_for_chunk(
    client: &mut RaknetClient,
    limits: CodecLimits,
    expected: (i32, i32),
    phase: &'static str,
) -> Result<(), Box<dyn Error>> {
    loop {
        let payload = next_payload(client)
            .await
            .map_err(|error| -> Box<dyn Error> {
                format!("waiting for {phase} chunk {expected:?}: {error}").into()
            })?;
        for packet in raw_packets(&payload, limits)? {
            if packet.id() != cobblestone_protocol84::FULL_CHUNK_DATA_ID || packet.body().len() < 8
            {
                continue;
            }
            let body = packet.body().as_slice();
            let chunk = (
                i32::from_be_bytes(body[0..4].try_into()?),
                i32::from_be_bytes(body[4..8].try_into()?),
            );
            if chunk == expected {
                return Ok(());
            }
        }
    }
}

async fn verify_stream_torture(
    client: &mut RaknetClient,
    limits: CodecLimits,
) -> Result<(), Box<dyn Error>> {
    for position in [
        [145.0, 64.0, 129.0],
        [161.0, 64.0, 129.0],
        [193.0, 64.0, 129.0],
    ] {
        send_movement(client, limits, position).await?;
    }
    wait_for_chunk(client, limits, (14, 8), "rapid movement").await?;
    println!("world-sync-client: stream-torture rapid=verified");

    send_movement(client, limits, [209.0, 64.0, 161.0]).await?;
    wait_for_chunk(client, limits, (15, 12), "diagonal movement").await?;
    println!("world-sync-client: stream-torture diagonal=verified");

    send_movement(client, limits, [129.0, 64.0, 129.0]).await?;
    wait_for_chunk(client, limits, (6, 6), "return movement").await?;
    println!("world-sync-client: stream-torture returned=verified");
    std::io::stdout().flush()?;
    tokio::time::sleep(Duration::from_millis(200)).await;

    Ok(())
}

async fn send_boundary_movement(
    client: &mut RaknetClient,
    limits: CodecLimits,
) -> Result<(), Box<dyn Error>> {
    let movement = RawPacket::new(
        packet_id::MOVE_PLAYER,
        move_player_body([145.0, 64.0, 129.0]),
    );
    let movement_frame = encode_game_frame(&movement, limits)?;
    client
        .send_with_options(
            Bytes::copy_from_slice(movement_frame.as_slice()),
            ClientSendOptions {
                reliability: Reliability::UnreliableSequenced,
                ..ClientSendOptions::default()
            },
        )
        .await?;
    let mut entering = std::collections::BTreeSet::new();
    while entering.len() < 5 {
        let payload = next_payload(client)
            .await
            .map_err(|error| -> Box<dyn Error> {
                format!("waiting for entering chunks: {error}").into()
            })?;
        for packet in raw_packets(&payload, limits)? {
            if packet.id() != cobblestone_protocol84::FULL_CHUNK_DATA_ID || packet.body().len() < 8
            {
                continue;
            }
            let body = packet.body().as_slice();
            let chunk_x = i32::from_be_bytes(body[0..4].try_into()?);
            let chunk_z = i32::from_be_bytes(body[4..8].try_into()?);
            if chunk_x == 11 && (6..=10).contains(&chunk_z) {
                entering.insert(chunk_z);
            }
        }
    }

    if entering != std::collections::BTreeSet::from([6, 7, 8, 9, 10]) {
        return Err(format!("unexpected entering chunk set: {entering:?}").into());
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr: SocketAddr = std::env::args()
        .nth(1)
        .ok_or("missing server address")?
        .parse()?;
    let send_movement_after_update =
        std::env::args().any(|argument| argument == "--move-after-update");
    let transition_only = std::env::args().any(|argument| argument == "--transition-only");
    let radius_cycle = std::env::args().any(|argument| argument == "--radius-cycle");
    let wide_initial = std::env::args().any(|argument| argument == "--initial-radius=3");
    let stream_torture = std::env::args().any(|argument| argument == "--stream-torture");
    let persistent_stream = std::env::args().any(|argument| argument == "--persistent-stream");
    let spawn_only = std::env::args().any(|argument| argument == "--spawn-only");
    let expected_spawn = std::env::args().find_map(|argument| {
        let value = argument.strip_prefix("--expect-spawn=")?;
        let mut parts = value.split(',');
        let x = parts.next()?.parse::<i32>().ok()?;
        let y = parts.next()?.parse::<i32>().ok()?;
        let z = parts.next()?.parse::<i32>().ok()?;
        parts.next().is_none().then_some([x, y, z])
    });
    let expect_nether = std::env::args().any(|argument| argument == "--expect-nether");
    let hold_east = std::env::args().any(|argument| argument == "--hold-east");
    let hold_west = std::env::args().any(|argument| argument == "--hold-west");
    let pending_disconnect = std::env::args().any(|argument| argument == "--pending-disconnect");
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
            if packet.id() == packet_id::START_GAME {
                let frame = encode_game_frame(&packet, limits)?;
                let BootstrapPacket::StartGame(start) =
                    decode_bootstrap_frame(frame.as_slice(), limits)?
                else {
                    return Err("StartGame packet decoded as wrong bootstrap variant".into());
                };
                let expected = if expect_nether {
                    DimensionId::Nether
                } else {
                    DimensionId::Overworld
                };
                if start.dimension != expected {
                    return Err(format!(
                        "unexpected StartGame dimension: expected {expected:?}, got {:?}",
                        start.dimension
                    )
                    .into());
                }
                if let Some(expected_spawn) = expected_spawn {
                    if start.spawn != expected_spawn {
                        return Err(format!(
                            "unexpected StartGame spawn: expected {expected_spawn:?}, got {:?}",
                            start.spawn
                        )
                        .into());
                    }
                    let expected_position = [
                        expected_spawn[0] as f32 + 0.5,
                        expected_spawn[1] as f32,
                        expected_spawn[2] as f32 + 0.5,
                    ];
                    if start.position != expected_position {
                        return Err(format!(
                            "unexpected StartGame position: expected {expected_position:?}, got {:?}",
                            start.position
                        )
                        .into());
                    }
                    println!(
                        "world-sync-client: spawn-position=verified x={} y={} z={}",
                        expected_spawn[0], expected_spawn[1], expected_spawn[2]
                    );
                }
                if expect_nether {
                    println!("world-sync-client: dimension=nether verified");
                }
                saw_start_game = true;
            }
            saw_adventure |= packet.id() == packet_id::ADVENTURE_SETTINGS;
        }
    }

    let requested_radius: i32 = if wide_initial { 3 } else { 2 };
    let request = RawPacket::new(
        REQUEST_CHUNK_RADIUS_ID,
        NativeBuffer::copy_from_slice(&requested_radius.to_be_bytes()),
    );
    let request_frame = encode_game_frame(&request, limits)?;
    send_frame(&mut client, request_frame.as_slice()).await?;

    let mut spawned = false;
    let mut initial_radius_ack = false;
    let mut initial_chunks = std::collections::BTreeSet::new();
    while !spawned {
        let payload = next_payload(&mut client).await?;
        for packet in raw_packets(&payload, limits)? {
            if packet.id() == CHUNK_RADIUS_UPDATED_ID {
                initial_radius_ack |= packet.body().as_slice() == requested_radius.to_be_bytes();
            }
            if packet.id() == cobblestone_protocol84::FULL_CHUNK_DATA_ID && packet.body().len() >= 8
            {
                let body = packet.body().as_slice();
                initial_chunks.insert((
                    i32::from_be_bytes(body[0..4].try_into()?),
                    i32::from_be_bytes(body[4..8].try_into()?),
                ));
            }
            if packet.id() == packet_id::PLAY_STATUS {
                let frame = encode_game_frame(&packet, limits)?;
                if let BootstrapPacket::PlayStatus(status) =
                    decode_bootstrap_frame(frame.as_slice(), limits)?
                {
                    spawned |= status.status() == 3;
                }
            }
        }
    }
    if wide_initial {
        let center = expected_spawn
            .map(|spawn| (spawn[0].div_euclid(16), spawn[2].div_euclid(16)))
            .unwrap_or((8, 8));
        let expected = (-3..=3)
            .flat_map(|dx| (-3..=3).map(move |dz| (center.0 + dx, center.1 + dz)))
            .collect::<std::collections::BTreeSet<_>>();
        if !initial_radius_ack || initial_chunks != expected {
            return Err(format!(
                "initial radius 3 view mismatch: ack={initial_radius_ack} chunks={} expected=49",
                initial_chunks.len(),
            )
            .into());
        }
        println!("world-sync-client: initial-radius=verified radius=3 chunks=49");
    }

    if spawn_only {
        println!("world-sync-client: spawn=verified");
        client.disconnect(None).await?;
        tokio::time::sleep(Duration::from_millis(100)).await;
        return Ok(());
    }

    if transition_only {
        send_boundary_movement(&mut client, limits).await?;
        println!("world-sync-client: transition=verified entering=5");
        client.disconnect(None).await?;
        return Ok(());
    }

    if radius_cycle {
        verify_radius_cycle(&mut client, limits).await?;
        println!("world-sync-client: radius-cycle=verified grow=24 shrink=1");
        client.disconnect(None).await?;
        return Ok(());
    }

    if stream_torture {
        verify_stream_torture(&mut client, limits).await?;
        println!("world-sync-client: stream-torture=verified");
        client.disconnect(None).await?;
        return Ok(());
    }

    if hold_east || hold_west {
        let (position, expected_chunk, marker) = if hold_east {
            ([161.0, 64.0, 129.0], (12, 8), "east")
        } else {
            ([97.0, 64.0, 129.0], (4, 8), "west")
        };
        send_movement(&mut client, limits, position).await?;
        wait_for_chunk(&mut client, limits, expected_chunk, "shared residency").await?;
        println!("world-sync-client: shared-view {marker}=ready");
        std::io::stdout().flush()?;
        tokio::time::sleep(Duration::from_millis(750)).await;
        client.disconnect(None).await?;
        tokio::time::sleep(Duration::from_millis(100)).await;
        return Ok(());
    }

    if pending_disconnect {
        send_movement(&mut client, limits, [145.0, 64.0, 129.0]).await?;
        println!("world-sync-client: pending-disconnect movement=sent");
        std::io::stdout().flush()?;
        tokio::time::sleep(Duration::from_secs(2)).await;
        return Ok(());
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

            if send_movement_after_update {
                send_movement(&mut client, limits, [145.0, 64.0, 129.0]).await?;
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            println!("world-sync-client: update=verified packet=0x13 x=128 y=5 z=128 state=0x010");

            if persistent_stream {
                send_movement(&mut client, limits, [161.0, 64.0, 129.0]).await?;
                wait_for_chunk(&mut client, limits, (12, 8), "persisted streaming").await?;
                println!("world-sync-client: persistent-stream persisted=verified");

                send_movement(&mut client, limits, [177.0, 64.0, 129.0]).await?;
                wait_for_chunk(&mut client, limits, (13, 8), "missing streaming").await?;
                println!("world-sync-client: persistent-stream generated=verified");
                std::io::stdout().flush()?;
                tokio::time::sleep(Duration::from_millis(200)).await;
            }

            client.disconnect(None).await?;
            tokio::time::sleep(Duration::from_millis(100)).await;
            return Ok(());
        }
    }
}
