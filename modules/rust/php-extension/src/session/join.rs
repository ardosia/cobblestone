use cobblestone_codec::{
    AdventureFlags, AdventureSettingsPacket, BatchPacket, BootstrapPacket, CHUNK_BLOCK_COUNT,
    CHUNK_COLUMN_COUNT, CHUNK_NIBBLE_BYTES, PlayStatusPacket, Protocol84ChunkSnapshot, RawPacket,
    SetDifficultyPacket, SetSpawnPositionPacket, SetTimePacket, StartGamePacket,
    decode_bootstrap_packet, encode_bootstrap_packet, encode_protocol84_full_chunk_data, packet_id,
};
use cobblestone_core::{NativeBuffer, RuntimeId};
use cobblestone_session::{SessionDelivery, SessionId, SessionPacket};
use ext_php_rs::binary::Binary;
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::{php_boundary, php_error};
use crate::runtime::current_runtime_id;
use crate::session::bridge::{codec_limits, owner_session_id, with_runtime};

const MAX_INITIAL_CHUNK_RADIUS: i32 = 3;
const MAX_INITIAL_CHUNKS: usize = 49;
const MAX_PROJECTION_BYTES: usize = 4 * 1024 * 1024;
const CHUNK_RADIUS_UPDATED_ID: u8 = 0x3e;

pub(crate) struct WorldBootstrap {
    pub(crate) seed: i32,
    pub(crate) generator: i32,
    pub(crate) spawn: [i32; 3],
    pub(crate) position: [f32; 3],
    pub(crate) time: i32,
    pub(crate) time_started: bool,
    pub(crate) level_id: String,
}

struct ProjectionReader<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> ProjectionReader<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }

    fn read_exact(&mut self, len: usize) -> PhpResult<&'a [u8]> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| php_error("chunk projection offset overflow"))?;
        if end > self.input.len() {
            return Err(php_error(format!(
                "truncated chunk projection: needed {len} bytes with {} remaining",
                self.input.len().saturating_sub(self.offset)
            )));
        }
        let bytes = &self.input[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }

    fn read_u16_le(&mut self) -> PhpResult<u16> {
        let bytes = self.read_exact(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32_le(&mut self) -> PhpResult<u32> {
        let bytes = self.read_exact(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_i32_le(&mut self) -> PhpResult<i32> {
        let bytes = self.read_exact(4)?;
        Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn finish(self) -> PhpResult<()> {
        if self.offset == self.input.len() {
            Ok(())
        } else {
            Err(php_error(format!(
                "chunk projection has {} trailing bytes",
                self.input.len() - self.offset
            )))
        }
    }
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
            dimension: 0,
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

fn decode_initial_chunk_projection(
    input: &[u8],
    expected_chunks: usize,
) -> PhpResult<Vec<RawPacket>> {
    if input.len() > MAX_PROJECTION_BYTES {
        return Err(php_error(format!(
            "initial chunk projection exceeds {MAX_PROJECTION_BYTES} bytes"
        )));
    }

    let mut reader = ProjectionReader::new(input);
    let declared_chunks = usize::try_from(reader.read_u32_le()?)
        .map_err(|_| php_error("initial chunk count exceeds platform size"))?;
    if declared_chunks != expected_chunks {
        return Err(php_error(format!(
            "initial chunk projection count mismatch: expected {expected_chunks}, got {declared_chunks}"
        )));
    }
    if declared_chunks > MAX_INITIAL_CHUNKS {
        return Err(php_error(format!(
            "initial chunk projection exceeds {MAX_INITIAL_CHUNKS} chunks"
        )));
    }

    let mut packets = Vec::with_capacity(declared_chunks);
    for _ in 0..declared_chunks {
        let chunk_x = reader.read_i32_le()?;
        let chunk_z = reader.read_i32_le()?;
        let block_ids = reader.read_exact(CHUNK_BLOCK_COUNT)?;
        let block_data = reader.read_exact(CHUNK_NIBBLE_BYTES)?;
        let sky_light = reader.read_exact(CHUNK_NIBBLE_BYTES)?;
        let block_light = reader.read_exact(CHUNK_NIBBLE_BYTES)?;
        let biomes = reader.read_exact(CHUNK_COLUMN_COUNT)?;
        let height_map = reader.read_exact(CHUNK_COLUMN_COUNT)?;

        let extra_count = usize::try_from(reader.read_u32_le()?)
            .map_err(|_| php_error("chunk extra-data count exceeds platform size"))?;
        if extra_count > CHUNK_BLOCK_COUNT {
            return Err(php_error(
                "chunk extra-data count exceeds fixed-target block count",
            ));
        }
        let mut extra_data = Vec::with_capacity(extra_count);
        for _ in 0..extra_count {
            extra_data.push((reader.read_u32_le()?, reader.read_u16_le()?));
        }

        let snapshot = Protocol84ChunkSnapshot {
            chunk_x,
            chunk_z,
            block_ids,
            block_data,
            sky_light,
            block_light,
            biomes,
            height_map,
            extra_data: &extra_data,
        };
        packets.push(
            encode_protocol84_full_chunk_data(snapshot)
                .map_err(|error| php_error(error.to_string()))?,
        );
    }
    reader.finish()?;
    Ok(packets)
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
        let spawn_y = i32_field("spawn y", spawn_y)?;
        if !(0..=127).contains(&spawn_y) {
            return Err(php_error("protocol-84 spawn y must be in range 0..127"));
        }

        let spawn_x = i32_field("spawn x", spawn_x)?;
        let spawn_z = i32_field("spawn z", spawn_z)?;
        let bootstrap = WorldBootstrap {
            seed: i32_field("world seed", seed)?,
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
        if !(1..=MAX_INITIAL_CHUNK_RADIUS).contains(&effective_radius) {
            return Err(php_error(format!(
                "effective initial chunk radius must be in range 1..={MAX_INITIAL_CHUNK_RADIUS}"
            )));
        }

        let side = effective_radius
            .checked_mul(2)
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| php_error("initial chunk radius overflow"))?;
        let expected_chunks = usize::try_from(side * side)
            .map_err(|_| php_error("initial chunk count exceeds platform size"))?;
        let projection: Vec<u8> = projection.into();
        let chunks = decode_initial_chunk_projection(&projection, expected_chunks)?;

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
}
