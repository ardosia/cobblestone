use std::error::Error;
use std::net::SocketAddr;
use std::num::NonZeroUsize;

use bytes::Bytes;
use cobblestone_codec::{
    AdventureFlags, AdventureSettingsPacket, BootstrapPacket, CodecError, CodecLimits, LoginPacket,
    PlayStatusPacket, SetDifficultyPacket, SetSpawnPositionPacket, SetTimePacket, StartGamePacket,
    decode_bootstrap_frame, decode_game_frame, encode_bootstrap_frame, encode_game_frame, packet_id,
};
use cobblestone_network::{Connection, NetworkConfig, NetworkServer, Reliability};

const DEFAULT_BIND: &str = "0.0.0.0:19132";
const MAX_CONNECTIONS: usize = 20;

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
    format!("MCPE;Cobblestone;{};;0;{MAX_CONNECTIONS}", cobblestone_codec::PROTOCOL_VERSION)
}

fn find_login(payload: &[u8], limits: CodecLimits) -> Result<Option<LoginPacket>, CodecError> {
    let raw = decode_game_frame(payload, limits)?;

    if raw.id() == packet_id::LOGIN {
        return match decode_bootstrap_frame(payload, limits)? {
            BootstrapPacket::Login(login) => Ok(Some(login)),
            _ => Ok(None),
        };
    }

    if raw.id() != packet_id::BATCH {
        return Ok(None);
    }

    let batch_frame = encode_game_frame(&raw, limits)?;
    let BootstrapPacket::Batch(batch) = decode_bootstrap_frame(batch_frame.as_slice(), limits)?
    else {
        return Ok(None);
    };

    for inner in batch.packets() {
        if inner.id() != packet_id::LOGIN {
            continue;
        }

        let frame = encode_game_frame(inner, limits)?;
        if let BootstrapPacket::Login(login) = decode_bootstrap_frame(frame.as_slice(), limits)? {
            return Ok(Some(login));
        }
    }

    Ok(None)
}

fn packet_ids(payload: &[u8], limits: CodecLimits) -> Result<Vec<u8>, CodecError> {
    let raw = decode_game_frame(payload, limits)?;
    if raw.id() != packet_id::BATCH {
        return Ok(vec![raw.id()]);
    }

    let frame = encode_game_frame(&raw, limits)?;
    let BootstrapPacket::Batch(batch) = decode_bootstrap_frame(frame.as_slice(), limits)? else {
        return Ok(vec![packet_id::BATCH]);
    };
    Ok(batch.packets().iter().map(|packet| packet.id()).collect())
}

async fn send_packet(
    connection: &Connection,
    packet: BootstrapPacket,
    limits: CodecLimits,
) -> Result<(), Box<dyn Error>> {
    let frame = encode_bootstrap_frame(&packet, limits)?;
    connection
        .send(
            Bytes::copy_from_slice(frame.as_slice()),
            Reliability::ReliableOrdered,
        )
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
            dimension: 0,
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let bind = std::env::var("COBBLESTONE_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_owned());
    let bind_addr: SocketAddr = bind.parse()?;
    let max_connections = NonZeroUsize::new(MAX_CONNECTIONS).expect("fixed nonzero max connections");
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
    use super::advertisement;

    #[test]
    fn advertisement_matches_fixed_target_layout() {
        assert_eq!(advertisement(), "MCPE;Cobblestone;84;;0;20");
    }
}
