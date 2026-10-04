use std::error::Error;
use std::net::SocketAddr;
use std::num::NonZeroUsize;

use bytes::Bytes;
use cobblestone_protocol84::{
    AdventureFlags, AdventureSettingsPacket, BatchPacket, BootstrapPacket, CodecError, CodecLimits,
    DimensionId, LoginPacket, PlayStatusPacket, RawPacket, SetDifficultyPacket,
    SetSpawnPositionPacket, SetTimePacket, StartGamePacket, decode_bootstrap_frame,
    decode_game_frame, encode_bootstrap_frame, encode_game_frame, packet_id,
};
use cobblestone_runtime::NativeBuffer;
use cobblestone_transport::{Connection, NetworkConfig, NetworkServer, Reliability};

const DEFAULT_BIND: &str = "0.0.0.0:19132";
const MAX_CONNECTIONS: usize = 20;
const PROBE_CHUNK_RADIUS: i32 = 2;
const FULL_CHUNK_DATA_ID: u8 = 0x34;
const REQUEST_CHUNK_RADIUS_ID: u8 = 0x3d;
const CHUNK_RADIUS_UPDATED_ID: u8 = 0x3e;
const CHUNK_ORDER_LAYERED: u8 = 1;

fn codec_limits() -> CodecLimits {
    CodecLimits::new(
        2 * 1024 * 1024,
        2 * 1024 * 1024,
        2 * 1024 * 1024,
        4 * 1024 * 1024,
        2 * 1024 * 1024,
        256,
    )
}

fn advertisement() -> String {
    format!(
        "MCPE;Cobblestone;{};;0;{MAX_CONNECTIONS}",
        cobblestone_protocol84::PROTOCOL_VERSION
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

fn find_login(payload: &[u8], limits: CodecLimits) -> Result<Option<LoginPacket>, CodecError> {
    for raw in raw_packets(payload, limits)? {
        if raw.id() != packet_id::LOGIN {
            continue;
        }

        let frame = encode_game_frame(&raw, limits)?;
        if let BootstrapPacket::Login(login) = decode_bootstrap_frame(frame.as_slice(), limits)? {
            return Ok(Some(login));
        }
    }

    Ok(None)
}

fn packet_ids(payload: &[u8], limits: CodecLimits) -> Result<Vec<u8>, CodecError> {
    Ok(raw_packets(payload, limits)?
        .iter()
        .map(RawPacket::id)
        .collect())
}

fn requested_chunk_radius(payload: &[u8], limits: CodecLimits) -> Result<Option<i32>, CodecError> {
    for raw in raw_packets(payload, limits)? {
        if raw.id() != REQUEST_CHUNK_RADIUS_ID {
            continue;
        }

        let bytes = raw.body().as_slice();
        if bytes.len() < 4 {
            return Err(CodecError::UnexpectedEof {
                needed: 4,
                remaining: bytes.len(),
            });
        }
        if bytes.len() > 4 {
            return Err(CodecError::TrailingBytes {
                remaining: bytes.len() - 4,
            });
        }

        return Ok(Some(i32::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
        ])));
    }

    Ok(None)
}

async fn send_packet(
    connection: &Connection,
    packet: BootstrapPacket,
    limits: CodecLimits,
) -> Result<(), Box<dyn Error>> {
    let frame = encode_bootstrap_frame(&packet, limits)?;
    send_frame(connection, frame.as_slice()).await
}

async fn send_raw_packet(
    connection: &Connection,
    packet: &RawPacket,
    limits: CodecLimits,
) -> Result<(), Box<dyn Error>> {
    let frame = encode_game_frame(packet, limits)?;
    send_frame(connection, frame.as_slice()).await
}

async fn send_frame(connection: &Connection, frame: &[u8]) -> Result<(), Box<dyn Error>> {
    connection
        .send(Bytes::copy_from_slice(frame), Reliability::ReliableOrdered)
        .await?;
    Ok(())
}

async fn send_initial_bootstrap(
    connection: &Connection,
    limits: CodecLimits,
) -> Result<(), Box<dyn Error>> {
    send_packet(
        connection,
        BootstrapPacket::PlayStatus(PlayStatusPacket::new(PlayStatusPacket::LOGIN_SUCCESS)),
        limits,
    )
    .await?;

    send_packet(
        connection,
        BootstrapPacket::StartGame(StartGamePacket {
            seed: -1,
            dimension: DimensionId::Overworld,
            generator: 1,
            gamemode: 0,
            entity_id: 0,
            spawn: [0, 64, 0],
            position: [0.5, 65.0, 0.5],
            level_id: "Cobblestone".to_owned(),
        }),
        limits,
    )
    .await?;

    send_packet(
        connection,
        BootstrapPacket::SetTime(SetTimePacket::new(0, true)),
        limits,
    )
    .await?;

    send_packet(
        connection,
        BootstrapPacket::SetSpawnPosition(SetSpawnPositionPacket::new(0, 64, 0)),
        limits,
    )
    .await?;

    send_packet(
        connection,
        BootstrapPacket::SetDifficulty(SetDifficultyPacket::new(1)),
        limits,
    )
    .await?;

    send_packet(
        connection,
        BootstrapPacket::AdventureSettings(AdventureSettingsPacket::new(
            AdventureFlags::SURVIVAL.bits() as i32,
            2,
            2,
        )),
        limits,
    )
    .await?;

    Ok(())
}

fn chunk_radius_updated(radius: i32) -> RawPacket {
    RawPacket::new(
        CHUNK_RADIUS_UPDATED_ID,
        NativeBuffer::copy_from_slice(&radius.to_be_bytes()),
    )
}

fn empty_layered_chunk_payload() -> NativeBuffer {
    const BLOCK_IDS: usize = 16 * 16 * 128;
    const NIBBLE_ARRAY: usize = BLOCK_IDS / 2;
    const HEIGHT_MAP: usize = 16 * 16;
    const BIOME_COLORS: usize = 16 * 16 * 4;
    const EXTRA_DATA_COUNT: usize = 4;
    const TOTAL: usize =
        BLOCK_IDS + NIBBLE_ARRAY * 3 + HEIGHT_MAP + BIOME_COLORS + EXTRA_DATA_COUNT;

    let mut payload = Vec::with_capacity(TOTAL);
    payload.resize(BLOCK_IDS, 0);
    payload.resize(payload.len() + NIBBLE_ARRAY, 0);
    payload.resize(payload.len() + NIBBLE_ARRAY, 0xff);
    payload.resize(payload.len() + NIBBLE_ARRAY, 0);
    payload.resize(payload.len() + HEIGHT_MAP, 0);
    payload.resize(payload.len() + BIOME_COLORS, 0);
    payload.extend_from_slice(&0_u32.to_le_bytes());

    debug_assert_eq!(payload.len(), TOTAL);
    NativeBuffer::from_vec(payload)
}

fn full_chunk_packet(chunk_x: i32, chunk_z: i32, payload: &NativeBuffer) -> RawPacket {
    let mut body = Vec::with_capacity(13 + payload.len());
    body.extend_from_slice(&chunk_x.to_be_bytes());
    body.extend_from_slice(&chunk_z.to_be_bytes());
    body.push(CHUNK_ORDER_LAYERED);
    body.extend_from_slice(
        &u32::try_from(payload.len())
            .expect("probe chunk payload fits protocol-84 length")
            .to_be_bytes(),
    );
    body.extend_from_slice(payload.as_slice());
    RawPacket::new(FULL_CHUNK_DATA_ID, NativeBuffer::from_vec(body))
}

async fn send_probe_chunks(
    connection: &Connection,
    limits: CodecLimits,
) -> Result<(), Box<dyn Error>> {
    send_raw_packet(
        connection,
        &chunk_radius_updated(PROBE_CHUNK_RADIUS),
        limits,
    )
    .await?;

    let payload = empty_layered_chunk_payload();
    let mut chunks = Vec::new();
    for chunk_x in -PROBE_CHUNK_RADIUS..=PROBE_CHUNK_RADIUS {
        for chunk_z in -PROBE_CHUNK_RADIUS..=PROBE_CHUNK_RADIUS {
            chunks.push(full_chunk_packet(chunk_x, chunk_z, &payload));
        }
    }

    send_packet(
        connection,
        BootstrapPacket::Batch(BatchPacket::new(chunks)),
        limits,
    )
    .await?;

    send_packet(
        connection,
        BootstrapPacket::PlayStatus(PlayStatusPacket::new(PlayStatusPacket::PLAYER_SPAWN)),
        limits,
    )
    .await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let bind = std::env::var("COBBLESTONE_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_owned());
    let bind_addr: SocketAddr = bind.parse()?;
    let max_connections =
        NonZeroUsize::new(MAX_CONNECTIONS).expect("fixed nonzero max connections");
    let limits = codec_limits();

    let config = NetworkConfig::protocol8(bind_addr, max_connections, advertisement());
    let mut server = NetworkServer::bind(config).await?;

    println!("cobblestone-client-bootstrap: listening={bind_addr} protocol=84 raknet=8");
    println!("cobblestone-client-bootstrap: waiting for one real client");

    let mut connection = server.accept().await?;
    println!(
        "cobblestone-client-bootstrap: raknet-connected peer={}",
        connection.peer_addr()
    );

    let mut bootstrapped = false;
    let mut chunks_sent = false;
    loop {
        let payload = match connection.recv().await {
            Ok(payload) => payload,
            Err(error) => {
                println!("cobblestone-client-bootstrap: connection-ended error={error}");
                break;
            }
        };

        if !bootstrapped {
            match find_login(&payload, limits) {
                Ok(Some(login)) => {
                    println!(
                        "cobblestone-client-bootstrap: login protocol={} chain_bytes={} skin_jwt_bytes={}",
                        login.protocol(),
                        login.chain_data().len(),
                        login.skin_jwt().len()
                    );
                    send_initial_bootstrap(&connection, limits).await?;
                    bootstrapped = true;
                    println!(
                        "cobblestone-client-bootstrap: sent login-success/start-game/time/spawn/difficulty/adventure"
                    );
                }
                Ok(None) => match packet_ids(&payload, limits) {
                    Ok(ids) => println!(
                        "cobblestone-client-bootstrap: pre-login protocol84 packet_ids={ids:?}"
                    ),
                    Err(error) => println!(
                        "cobblestone-client-bootstrap: pre-login undecodable payload bytes={} error={error}",
                        payload.len()
                    ),
                },
                Err(error) => println!(
                    "cobblestone-client-bootstrap: login-decode-failed bytes={} error={error}",
                    payload.len()
                ),
            }
            continue;
        }

        if !chunks_sent {
            match requested_chunk_radius(&payload, limits) {
                Ok(Some(requested)) => {
                    println!(
                        "cobblestone-client-bootstrap: chunk-radius requested={requested} probe_radius={PROBE_CHUNK_RADIUS}"
                    );
                    send_probe_chunks(&connection, limits).await?;
                    chunks_sent = true;
                    println!(
                        "cobblestone-client-bootstrap: sent chunk-radius-ack chunks=25 player-spawn"
                    );
                    continue;
                }
                Ok(None) => {}
                Err(error) => println!(
                    "cobblestone-client-bootstrap: chunk-radius decode failed bytes={} error={error}",
                    payload.len()
                ),
            }
        }

        match packet_ids(&payload, limits) {
            Ok(ids) => println!(
                "cobblestone-client-bootstrap: post-bootstrap packet_ids={ids:?} bytes={}",
                payload.len()
            ),
            Err(error) => println!(
                "cobblestone-client-bootstrap: post-bootstrap undecodable bytes={} error={error}",
                payload.len()
            ),
        }
    }

    server.shutdown().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        PROBE_CHUNK_RADIUS, advertisement, chunk_radius_updated, empty_layered_chunk_payload,
        full_chunk_packet,
    };

    #[test]
    fn advertisement_matches_fixed_target_layout() {
        assert_eq!(advertisement(), "MCPE;Cobblestone;84;;0;20");
    }

    #[test]
    fn empty_probe_chunk_matches_historical_layered_payload_size() {
        assert_eq!(empty_layered_chunk_payload().len(), 83_204);
    }

    #[test]
    fn probe_chunk_envelopes_use_fixed_target_ids_and_lengths() {
        let payload = empty_layered_chunk_payload();
        let chunk = full_chunk_packet(0, 0, &payload);
        assert_eq!(chunk.id(), 0x34);
        assert_eq!(chunk.body().len(), 83_217);

        let radius = chunk_radius_updated(PROBE_CHUNK_RADIUS);
        assert_eq!(radius.id(), 0x3e);
        assert_eq!(radius.body().as_slice(), &PROBE_CHUNK_RADIUS.to_be_bytes());
    }
}
