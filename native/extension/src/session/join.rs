mod projection;

use cobblestone_protocol84::{
    AdventureFlags, AdventureSettingsPacket, BatchPacket, BootstrapPacket, DimensionId,
    PlayStatusPacket, RawPacket, SetDifficultyPacket, SetSpawnPositionPacket, SetTimePacket,
    StartGamePacket, decode_bootstrap_packet, encode_bootstrap_packet, packet_id,
};
use cobblestone_runtime::{NativeBuffer, RuntimeId};
use cobblestone_session::{SessionDelivery, SessionId, SessionPacket};
use cobblestone_world::ChunkCoord;
use ext_php_rs::binary::Binary;
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::{php_boundary, php_error};
use crate::runtime::current_runtime_id;
use crate::session::bridge::{codec_limits, owner_session_id, with_runtime};
use crate::world::{protocol84_chunk, resolve_world};

const MAX_INITIAL_CHUNK_RADIUS: i32 = 3;
pub(super) const CHUNK_RADIUS_UPDATED_ID: u8 = 0x3e;

pub(crate) struct WorldBootstrap {
    pub(crate) seed: i32,
    pub(crate) dimension: DimensionId,
    pub(crate) generator: i32,
    pub(crate) spawn: [i32; 3],
    pub(crate) position: [f32; 3],
    pub(crate) time: i32,
    pub(crate) time_started: bool,
    pub(crate) level_id: String,
}

pub(crate) fn bootstrap_session_packet(packet: BootstrapPacket) -> PhpResult<SessionPacket> {
    let raw = encode_bootstrap_packet(&packet, codec_limits())
        .map_err(|error| php_error(error.to_string()))?;
    Ok(SessionPacket::new(raw.id(), raw.body().clone()))
}

pub(crate) fn validate_login_body(body: Vec<u8>) -> PhpResult<()> {
    let raw = RawPacket::new(packet_id::LOGIN, NativeBuffer::from_vec(body));
    match decode_bootstrap_packet(raw, codec_limits())
        .map_err(|error| php_error(error.to_string()))?
    {
        BootstrapPacket::Login(_) => Ok(()),
        _ => Err(php_error("expected protocol-84 Login packet")),
    }
}

pub(crate) fn initial_bootstrap_packets(
    bootstrap: &WorldBootstrap,
) -> PhpResult<Vec<SessionPacket>> {
    [
        BootstrapPacket::PlayStatus(PlayStatusPacket::new(PlayStatusPacket::LOGIN_SUCCESS)),
        BootstrapPacket::StartGame(StartGamePacket {
            seed: bootstrap.seed,
            dimension: bootstrap.dimension,
            generator: bootstrap.generator,
            gamemode: 0,
            entity_id: 0,
            spawn: bootstrap.spawn,
            position: bootstrap.position,
            level_id: bootstrap.level_id.clone(),
        }),
        BootstrapPacket::SetTime(SetTimePacket::new(bootstrap.time, bootstrap.time_started)),
        BootstrapPacket::SetSpawnPosition(SetSpawnPositionPacket::new(
            bootstrap.spawn[0],
            bootstrap.spawn[1],
            bootstrap.spawn[2],
        )),
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

pub(crate) fn requested_chunk_radius(body: &[u8]) -> PhpResult<i32> {
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

fn i32_field(field: &'static str, value: i64) -> PhpResult<i32> {
    i32::try_from(value).map_err(|_| php_error(format!("{field} must fit signed 32-bit range")))
}

pub(crate) fn queue_reliable_ordered(
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

/// Validates Login and queues protocol-84 bootstrap state projected from the PHP-owned World.
#[php_function]
#[php(name = "cobblestone_session_protocol84_accept_login_world")]
#[allow(clippy::too_many_arguments)]
pub fn cobblestone_session_protocol84_accept_login_world(
    session_id: i64,
    body: Binary<u8>,
    seed: i64,
    generator: i64,
    dimension: i64,
    spawn_x: i64,
    spawn_y: i64,
    spawn_z: i64,
    time: i64,
    time_started: bool,
    level_id: String,
) -> PhpResult<()> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let session_id = owner_session_id(session_id)?;
        validate_login_body(body.into())?;

        let generator = i32_field("generator", generator)?;
        if !(0..=2).contains(&generator) {
            return Err(php_error("protocol-84 generator id must be in range 0..2"));
        }
        let dimension = u8::try_from(dimension)
            .ok()
            .and_then(|value| DimensionId::try_from(value).ok())
            .ok_or_else(|| php_error("unsupported MCPE 0.15.10 dimension id"))?;
        let spawn_y = i32_field("spawn y", spawn_y)?;
        if !(0..=127).contains(&spawn_y) {
            return Err(php_error("protocol-84 spawn y must be in range 0..127"));
        }

        let spawn_x = i32_field("spawn x", spawn_x)?;
        let spawn_z = i32_field("spawn z", spawn_z)?;
        let bootstrap = WorldBootstrap {
            seed: i32_field("world seed", seed)?,
            dimension,
            generator,
            spawn: [spawn_x, spawn_y, spawn_z],
            position: [spawn_x as f32 + 0.5, spawn_y as f32, spawn_z as f32 + 0.5],
            time: i32_field("world time", time)?,
            time_started,
            level_id,
        };

        queue_reliable_ordered(owner, session_id, initial_bootstrap_packets(&bootstrap)?)
    })
}

/// Decodes one fixed-target RequestChunkRadius body without changing gameplay/world state.
#[php_function]
#[php(name = "cobblestone_session_protocol84_request_chunk_radius")]
pub fn cobblestone_session_protocol84_request_chunk_radius(body: Binary<u8>) -> PhpResult<i64> {
    php_boundary(|| {
        let _owner = current_runtime_id().map_err(php_error)?;
        let body: Vec<u8> = body.into();
        Ok(i64::from(requested_chunk_radius(&body)?))
    })
}

fn initial_chunk_count(effective_radius: i32) -> PhpResult<usize> {
    if !(1..=MAX_INITIAL_CHUNK_RADIUS).contains(&effective_radius) {
        return Err(php_error(format!(
            "effective initial chunk radius must be in range 1..={MAX_INITIAL_CHUNK_RADIUS}"
        )));
    }

    let side = effective_radius
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| php_error("initial chunk radius overflow"))?;
    usize::try_from(side * side).map_err(|_| php_error("initial chunk count exceeds platform size"))
}

fn queue_initial_chunk_batch(
    owner: RuntimeId,
    session_id: SessionId,
    effective_radius: i32,
    chunks: Vec<RawPacket>,
) -> PhpResult<i64> {
    let expected_chunks = initial_chunk_count(effective_radius)?;
    if chunks.len() != expected_chunks {
        return Err(php_error(format!(
            "initial native chunk count mismatch: expected {expected_chunks}, got {}",
            chunks.len()
        )));
    }

    let batch = bootstrap_session_packet(BootstrapPacket::Batch(BatchPacket::new(chunks)))?;
    let encoded_bytes = batch
        .body()
        .len()
        .checked_add(1)
        .and_then(|value| i64::try_from(value).ok())
        .ok_or_else(|| php_error("encoded chunk Batch length exceeds PHP integer range"))?;

    let packets = vec![
        SessionPacket::new(
            CHUNK_RADIUS_UPDATED_ID,
            NativeBuffer::copy_from_slice(&effective_radius.to_be_bytes()),
        ),
        batch,
        bootstrap_session_packet(BootstrapPacket::PlayStatus(PlayStatusPacket::new(
            PlayStatusPacket::PLAYER_SPAWN,
        )))?,
    ];
    queue_reliable_ordered(owner, session_id, packets)?;
    Ok(encoded_bytes)
}

/// Decodes the private PHP/native bulk projection, encodes exact protocol-84 chunks, and queues
/// ChunkRadiusUpdated + one compressed Batch + PLAYER_SPAWN.
///
/// Returns the compressed Batch packet size (packet id plus body) for owner-runtime observability.
#[php_function]
#[php(name = "cobblestone_session_protocol84_send_initial_chunks")]
pub fn cobblestone_session_protocol84_send_initial_chunks(
    session_id: i64,
    effective_radius: i64,
    projection: Binary<u8>,
) -> PhpResult<i64> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let session_id = owner_session_id(session_id)?;
        let effective_radius = i32_field("effective chunk radius", effective_radius)?;
        let expected_chunks = initial_chunk_count(effective_radius)?;
        let projection: Vec<u8> = projection.into();
        let chunks = projection::decode_initial_chunk_projection(&projection, expected_chunks)?;

        queue_initial_chunk_batch(owner, session_id, effective_radius, chunks)
    })
}

/// Reads immutable native world snapshots directly, reuses revision-keyed protocol-84 chunk
/// packets, and queues ChunkRadiusUpdated + one compressed Batch + PLAYER_SPAWN.
#[php_function]
#[php(name = "cobblestone_session_protocol84_send_native_chunks")]
pub fn cobblestone_session_protocol84_send_native_chunks(
    session_id: i64,
    effective_radius: i64,
    world_handle: i64,
    center_chunk_x: i64,
    center_chunk_z: i64,
) -> PhpResult<i64> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let session_id = owner_session_id(session_id)?;
        let effective_radius = i32_field("effective chunk radius", effective_radius)?;
        let expected_chunks = initial_chunk_count(effective_radius)?;
        let center_x = i32_field("center chunk x", center_chunk_x)?;
        let center_z = i32_field("center chunk z", center_chunk_z)?;
        let min_x = center_x
            .checked_sub(effective_radius)
            .ok_or_else(|| php_error("initial chunk x range underflow"))?;
        let max_x = center_x
            .checked_add(effective_radius)
            .ok_or_else(|| php_error("initial chunk x range overflow"))?;
        let min_z = center_z
            .checked_sub(effective_radius)
            .ok_or_else(|| php_error("initial chunk z range underflow"))?;
        let max_z = center_z
            .checked_add(effective_radius)
            .ok_or_else(|| php_error("initial chunk z range overflow"))?;

        let store = resolve_world(world_handle)?;
        let mut chunks = Vec::with_capacity(expected_chunks);
        let mut positions = Vec::with_capacity(expected_chunks);
        for x in min_x..=max_x {
            for z in min_z..=max_z {
                let position = ChunkCoord::new(x, z);
                chunks.push(protocol84_chunk(world_handle, position)?);
                positions.push(position);
            }
        }

        let mut pinned = Vec::with_capacity(positions.len());
        for &position in &positions {
            if let Err(error) = store.pin_chunk(position) {
                for &rollback in &pinned {
                    let _ = store.unpin_chunk(rollback);
                }
                return Err(php_error(error.to_string()));
            }
            pinned.push(position);
        }

        let encoded = match queue_initial_chunk_batch(owner, session_id, effective_radius, chunks) {
            Ok(encoded) => encoded,
            Err(error) => {
                for &position in &pinned {
                    let _ = store.unpin_chunk(position);
                }
                return Err(error);
            }
        };
        let cursor = store.current_change_sequence();
        super::view::install_view(
            (owner, session_id),
            world_handle,
            store.clone(),
            ChunkCoord::new(center_x, center_z),
            effective_radius,
            cursor,
            positions,
        );
        Ok(encoded)
    })
}

pub(crate) fn register(module: ModuleBuilder) -> ModuleBuilder {
    module
        .function(wrap_function!(
            cobblestone_session_protocol84_accept_login_world
        ))
        .function(wrap_function!(
            cobblestone_session_protocol84_request_chunk_radius
        ))
        .function(wrap_function!(
            cobblestone_session_protocol84_send_initial_chunks
        ))
        .function(wrap_function!(
            cobblestone_session_protocol84_send_native_chunks
        ))
}
