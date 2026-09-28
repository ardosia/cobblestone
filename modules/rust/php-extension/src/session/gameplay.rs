use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, MutexGuard};

use cobblestone_codec::{MovePlayerPacket, decode_protocol84_move_player};
use cobblestone_core::{ChunkCoord, RuntimeId};
use cobblestone_session::SessionId;
use ext_php_rs::binary::Binary;
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::{php_boundary, php_error};
use crate::runtime::current_runtime_id;
use crate::session::bridge::owner_session_id;

const CHUNK_EDGE: f32 = 16.0;
const MAX_PLAYER_COORDINATE: f32 = 1_000_000.0;

#[derive(Debug, Copy, Clone, PartialEq)]
struct PlayerState {
    position: [f32; 3],
    chunk: ChunkCoord,
}

static PLAYER_STATES: LazyLock<Mutex<HashMap<(RuntimeId, SessionId), PlayerState>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn player_states() -> MutexGuard<'static, HashMap<(RuntimeId, SessionId), PlayerState>> {
    match PLAYER_STATES.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn validate_position(position: [f32; 3]) -> Result<PlayerState, &'static str> {
    if position
        .iter()
        .any(|coordinate| !coordinate.is_finite() || coordinate.abs() > MAX_PLAYER_COORDINATE)
    {
        return Err(
            "protocol-84 MovePlayer position is non-finite or outside the supported world range",
        );
    }

    Ok(PlayerState {
        position,
        chunk: ChunkCoord::new(
            (position[0] / CHUNK_EDGE).floor() as i32,
            (position[2] / CHUNK_EDGE).floor() as i32,
        ),
    })
}

fn state_from_move(packet: MovePlayerPacket) -> Result<PlayerState, &'static str> {
    validate_position(packet.position())
}

fn spawn_state(spawn: [i32; 3]) -> Result<PlayerState, &'static str> {
    validate_position([
        spawn[0] as f32 + 0.5,
        spawn[1] as f32,
        spawn[2] as f32 + 0.5,
    ])
}

fn i32_field(field: &'static str, value: i64) -> PhpResult<i32> {
    i32::try_from(value).map_err(|_| php_error(format!("{field} must fit signed 32-bit range")))
}

pub(crate) fn forget_session(owner: RuntimeId, session_id: SessionId) {
    player_states().remove(&(owner, session_id));
}

pub(crate) fn forget_runtime(owner: RuntimeId) {
    player_states().retain(|(runtime, _), _| *runtime != owner);
}

/// Initializes post-spawn player position state without changing the streamed world view.
#[php_function]
#[php(name = "cobblestone_session_protocol84_player_spawned")]
pub fn cobblestone_session_protocol84_player_spawned(
    session_id: i64,
    spawn_x: i64,
    spawn_y: i64,
    spawn_z: i64,
) -> PhpResult<()> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let session_id = owner_session_id(session_id)?;
        let state = spawn_state([
            i32_field("spawn x", spawn_x)?,
            i32_field("spawn y", spawn_y)?,
            i32_field("spawn z", spawn_z)?,
        ])
        .map_err(php_error)?;

        player_states().insert((owner, session_id), state);
        Ok(())
    })
}

/// Decodes and tracks one post-spawn protocol-84 MovePlayer body.
///
/// This deliberately does not recenter the current chunk view yet.
#[php_function]
#[php(name = "cobblestone_session_protocol84_track_move_player")]
pub fn cobblestone_session_protocol84_track_move_player(
    session_id: i64,
    body: Binary<u8>,
) -> PhpResult<()> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let session_id = owner_session_id(session_id)?;
        let body: Vec<u8> = body.into();
        let packet =
            decode_protocol84_move_player(&body).map_err(|error| php_error(error.to_string()))?;
        let state = state_from_move(packet).map_err(php_error)?;

        let mut states = player_states();
        let current = states.get_mut(&(owner, session_id)).ok_or_else(|| {
            php_error("protocol-84 MovePlayer received before spawned player state")
        })?;
        *current = state;
        Ok(())
    })
}

pub(crate) fn register(module: ModuleBuilder) -> ModuleBuilder {
    module
        .function(wrap_function!(
            cobblestone_session_protocol84_player_spawned
        ))
        .function(wrap_function!(
            cobblestone_session_protocol84_track_move_player
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn move_packet_body(position: [f32; 3]) -> Vec<u8> {
        let mut body = Vec::with_capacity(34);
        body.extend_from_slice(&0_i64.to_be_bytes());
        for value in [position[0], position[1], position[2], 90.0, 45.0, 0.0] {
            body.extend_from_slice(&value.to_bits().to_be_bytes());
        }
        body.push(0);
        body.push(1);
        body
    }

    fn decode_state(position: [f32; 3]) -> Result<PlayerState, &'static str> {
        let body = move_packet_body(position);
        let packet = decode_protocol84_move_player(&body).expect("valid MovePlayer fixture");
        state_from_move(packet)
    }

    #[test]
    fn movement_tracks_floor_based_chunk_coordinates() {
        let positive = decode_state([16.0, 64.0, 31.999]).expect("positive position");
        assert_eq!(positive.position, [16.0, 64.0, 31.999]);
        assert_eq!(positive.chunk, ChunkCoord::new(1, 1));

        let negative = decode_state([-0.25, 64.0, -16.0]).expect("negative position");
        assert_eq!(negative.chunk, ChunkCoord::new(-1, -1));
    }

    #[test]
    fn spawned_position_matches_start_game_convention() {
        let state = spawn_state([-1, 64, 16]).expect("spawn position");
        assert_eq!(state.position, [-0.5, 64.0, 16.5]);
        assert_eq!(state.chunk, ChunkCoord::new(-1, 1));
    }

    #[test]
    fn movement_rejects_non_finite_and_out_of_range_positions() {
        assert!(decode_state([f32::NAN, 64.0, 0.0]).is_err());
        assert!(decode_state([0.0, f32::INFINITY, 0.0]).is_err());
        assert!(decode_state([MAX_PLAYER_COORDINATE + 1.0, 64.0, 0.0]).is_err());
        assert!(decode_state([0.0, 64.0, -MAX_PLAYER_COORDINATE - 1.0]).is_err());
    }
}
