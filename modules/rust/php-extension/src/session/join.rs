use cobblestone_codec::{
    AdventureFlags, AdventureSettingsPacket, BatchPacket, BootstrapPacket, PlayStatusPacket,
    RawPacket, SetDifficultyPacket, SetSpawnPositionPacket, SetTimePacket, StartGamePacket,
    decode_bootstrap_frame, decode_game_frame, encode_bootstrap_frame, encode_game_frame,
    packet_id,
};
use cobblestone_core::{NativeBuffer, RuntimeId};
use cobblestone_session::{SessionDelivery, SessionId, SessionPacket};
use ext_php_rs::binary::Binary;
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::{php_boundary, php_error};
use crate::runtime::current_runtime_id;
use crate::session::bridge::{codec_limits, owner_session_id, with_runtime};

const PROBE_CHUNK_RADIUS: i32 = 2;
const FULL_CHUNK_DATA_ID: u8 = 0x34;
const CHUNK_RADIUS_UPDATED_ID: u8 = 0x3e;
const CHUNK_ORDER_LAYERED: u8 = 1;

fn bootstrap_session_packet(packet: BootstrapPacket) -> PhpResult<SessionPacket> {
    let limits = codec_limits();
    let frame =
        encode_bootstrap_frame(&packet, limits).map_err(|error| php_error(error.to_string()))?;
    let raw = decode_game_frame(frame.as_slice(), limits)
        .map_err(|error| php_error(error.to_string()))?;
    Ok(SessionPacket::new(raw.id(), raw.body().clone()))
}

fn validate_login_body(body: Vec<u8>) -> PhpResult<()> {
    let limits = codec_limits();
    let raw = RawPacket::new(packet_id::LOGIN, NativeBuffer::from_vec(body));
    let frame = encode_game_frame(&raw, limits).map_err(|error| php_error(error.to_string()))?;

    match decode_bootstrap_frame(frame.as_slice(), limits)
        .map_err(|error| php_error(error.to_string()))?
    {
        BootstrapPacket::Login(_) => Ok(()),
        _ => Err(php_error("expected protocol-84 Login packet")),
    }
}

fn initial_bootstrap_packets() -> PhpResult<Vec<SessionPacket>> {
    [
        BootstrapPacket::PlayStatus(PlayStatusPacket::new(PlayStatusPacket::LOGIN_SUCCESS)),
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
        BootstrapPacket::SetTime(SetTimePacket::new(0, true)),
        BootstrapPacket::SetSpawnPosition(SetSpawnPositionPacket::new(0, 64, 0)),
        BootstrapPacket::SetDifficulty(SetDifficultyPacket::new(1)),
        BootstrapPacket::AdventureSettings(AdventureSettingsPacket::new(
            AdventureFlags::SURVIVAL.bits() as i32,
            2,
            2,
        )),
    ]
    .into_iter()
    .map(bootstrap_session_packet)
    .collect()
}

fn requested_chunk_radius(body: &[u8]) -> PhpResult<i32> {
    if body.len() != 4 {
        return Err(php_error(format!(
            "protocol-84 RequestChunkRadius body must be exactly 4 bytes, got {}",
            body.len()
        )));
    }

    let radius = i32::from_be_bytes([body[0], body[1], body[2], body[3]]);
    if radius <= 0 {
        return Err(php_error("protocol-84 chunk radius must be positive"));
    }
    Ok(radius)
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

    NativeBuffer::from_vec(payload)
}

fn full_chunk_packet(chunk_x: i32, chunk_z: i32, payload: &NativeBuffer) -> RawPacket {
    let mut body = Vec::with_capacity(13 + payload.len());
    body.extend_from_slice(&chunk_x.to_be_bytes());
    body.extend_from_slice(&chunk_z.to_be_bytes());
    body.push(CHUNK_ORDER_LAYERED);
    body.extend_from_slice(
        &u32::try_from(payload.len())
            .expect("fixed probe chunk payload fits protocol-84 length")
            .to_be_bytes(),
    );
    body.extend_from_slice(payload.as_slice());
    RawPacket::new(FULL_CHUNK_DATA_ID, NativeBuffer::from_vec(body))
}

fn spawn_probe_packets() -> PhpResult<Vec<SessionPacket>> {
    let mut packets = Vec::with_capacity(3);
    packets.push(SessionPacket::new(
        CHUNK_RADIUS_UPDATED_ID,
        NativeBuffer::copy_from_slice(&PROBE_CHUNK_RADIUS.to_be_bytes()),
    ));

    let payload = empty_layered_chunk_payload();
    let mut chunks = Vec::new();
    for chunk_x in -PROBE_CHUNK_RADIUS..=PROBE_CHUNK_RADIUS {
        for chunk_z in -PROBE_CHUNK_RADIUS..=PROBE_CHUNK_RADIUS {
            chunks.push(full_chunk_packet(chunk_x, chunk_z, &payload));
        }
    }
    packets.push(bootstrap_session_packet(BootstrapPacket::Batch(
        BatchPacket::new(chunks),
    ))?);
    packets.push(bootstrap_session_packet(BootstrapPacket::PlayStatus(
        PlayStatusPacket::new(PlayStatusPacket::PLAYER_SPAWN),
    ))?);

    Ok(packets)
}

fn queue_reliable_ordered(
    owner: RuntimeId,
    session_id: SessionId,
    packets: Vec<SessionPacket>,
) -> PhpResult<()> {
    for packet in packets {
        with_runtime(owner, |host| {
            host.try_send(session_id, packet, SessionDelivery::ReliableOrdered)
        })?;
    }
    Ok(())
}

/// Validates the fixed-target Login body and queues the initial protocol-84 bootstrap sequence.
///
/// This is kernel-internal compatibility machinery; gameplay/plugin APIs never call it directly.
#[php_function]
#[php(name = "cobblestone_session_protocol84_accept_login")]
pub fn cobblestone_session_protocol84_accept_login(
    session_id: i64,
    body: Binary<u8>,
) -> PhpResult<()> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let session_id = owner_session_id(session_id)?;
        validate_login_body(body.into())?;
        queue_reliable_ordered(owner, session_id, initial_bootstrap_packets()?)
    })
}

/// Decodes RequestChunkRadius and queues the bounded synthetic spawn probe.
///
/// Returns the client-requested radius for owner-runtime observability. The compatibility probe
/// remains capped at radius two until the real world/chunk system replaces it.
#[php_function]
#[php(name = "cobblestone_session_protocol84_spawn_probe")]
pub fn cobblestone_session_protocol84_spawn_probe(
    session_id: i64,
    body: Binary<u8>,
) -> PhpResult<i64> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let session_id = owner_session_id(session_id)?;
        let body: Vec<u8> = body.into();
        let requested = requested_chunk_radius(&body)?;
        queue_reliable_ordered(owner, session_id, spawn_probe_packets()?)?;
        Ok(i64::from(requested))
    })
}

pub(crate) fn register(module: ModuleBuilder) -> ModuleBuilder {
    module
        .function(wrap_function!(cobblestone_session_protocol84_accept_login))
        .function(wrap_function!(cobblestone_session_protocol84_spawn_probe))
}
