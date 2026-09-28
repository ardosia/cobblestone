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
use crate::session::join::{
    ChunkViewDelta, ViewChunkQueueResult, commit_view_delta, plan_view_delta,
    queue_view_delta_chunks,
};

const CHUNK_EDGE: f32 = 16.0;
const MAX_PLAYER_COORDINATE: f32 = 1_000_000.0;

#[derive(Debug, Clone, PartialEq)]
struct PlayerState {
    position: [f32; 3],
    chunk: ChunkCoord,
    view_delta: Option<ChunkViewDelta>,
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
        view_delta: None,
    })
}

fn state_from_move(packet: MovePlayerPacket) -> Result<PlayerState, &'static str> {
    validate_position(packet.position())
}

fn encode_view_delta(delta: Option<&ChunkViewDelta>) -> Result<Binary<u8>, &'static str> {
    let Some(delta) = delta else {
        return Ok(Binary::new(Vec::new()));
    };
    let entering_count =
        u32::try_from(delta.entering.len()).map_err(|_| "chunk view delta is too large")?;
    let mut projection = Vec::with_capacity(20 + delta.entering.len().saturating_mul(8));
    for coordinate in [
        delta.from_center.x(),
        delta.from_center.z(),
        delta.to_center.x(),
        delta.to_center.z(),
    ] {
        projection.extend_from_slice(&coordinate.to_le_bytes());
    }
    projection.extend_from_slice(&entering_count.to_le_bytes());
    for position in &delta.entering {
        projection.extend_from_slice(&position.x().to_le_bytes());
        projection.extend_from_slice(&position.z().to_le_bytes());
    }
    Ok(Binary::new(projection))
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
) -> PhpResult<Binary<u8>> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let session_id = owner_session_id(session_id)?;
        let body: Vec<u8> = body.into();
        let packet =
            decode_protocol84_move_player(&body).map_err(|error| php_error(error.to_string()))?;
        let mut state = state_from_move(packet).map_err(php_error)?;

        if !player_states().contains_key(&(owner, session_id)) {
            return Err(php_error(
                "protocol-84 MovePlayer received before spawned player state",
            ));
        }

        state.view_delta = plan_view_delta(owner, session_id, state.chunk);
        let projection = encode_view_delta(state.view_delta.as_ref()).map_err(php_error)?;
        player_states().insert((owner, session_id), state);
        Ok(projection)
    })
}

/// Commits one already-queued view transition into native view ownership.
#[php_function]
#[php(name = "cobblestone_session_protocol84_commit_prepared_view")]
pub fn cobblestone_session_protocol84_commit_prepared_view(
    session_id: i64,
    from_chunk_x: i64,
    from_chunk_z: i64,
    to_chunk_x: i64,
    to_chunk_z: i64,
) -> PhpResult<bool> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let session_id = owner_session_id(session_id)?;
        let from_center = ChunkCoord::new(
            i32_field("view transition from chunk x", from_chunk_x)?,
            i32_field("view transition from chunk z", from_chunk_z)?,
        );
        let to_center = ChunkCoord::new(
            i32_field("view transition to chunk x", to_chunk_x)?,
            i32_field("view transition to chunk z", to_chunk_z)?,
        );

        let delta = {
            let states = player_states();
            let state = states
                .get(&(owner, session_id))
                .ok_or_else(|| php_error("cannot commit chunk view before spawned player state"))?;
            state
                .view_delta
                .clone()
                .ok_or_else(|| php_error("player has no pending chunk view delta"))?
        };

        if delta.from_center != from_center || delta.to_center != to_center {
            return Err(php_error(
                "prepared chunk view transition no longer matches the pending player delta",
            ));
        }

        let committed = commit_view_delta(owner, session_id, &delta)?;
        if committed
            && let Some(state) = player_states().get_mut(&(owner, session_id))
            && state.view_delta.as_ref() == Some(&delta)
        {
            state.view_delta = None;
        }
        Ok(committed)
    })
}

/// Queues fully prepared entering chunks without recentering or releasing the old view.
#[php_function]
#[php(name = "cobblestone_session_protocol84_send_prepared_view_chunks")]
pub fn cobblestone_session_protocol84_send_prepared_view_chunks(
    session_id: i64,
    from_chunk_x: i64,
    from_chunk_z: i64,
    to_chunk_x: i64,
    to_chunk_z: i64,
) -> PhpResult<i64> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let session_id = owner_session_id(session_id)?;
        let from_center = ChunkCoord::new(
            i32_field("view transition from chunk x", from_chunk_x)?,
            i32_field("view transition from chunk z", from_chunk_z)?,
        );
        let to_center = ChunkCoord::new(
            i32_field("view transition to chunk x", to_chunk_x)?,
            i32_field("view transition to chunk z", to_chunk_z)?,
        );

        let delta = {
            let states = player_states();
            let state = states.get(&(owner, session_id)).ok_or_else(|| {
                php_error("cannot send entering chunks before spawned player state")
            })?;
            state
                .view_delta
                .clone()
                .ok_or_else(|| php_error("player has no pending chunk view delta"))?
        };

        if delta.from_center != from_center || delta.to_center != to_center {
            return Err(php_error(
                "prepared chunk view transition no longer matches the pending player delta",
            ));
        }

        Ok(match queue_view_delta_chunks(owner, session_id, &delta)? {
            ViewChunkQueueResult::Backpressured => 0,
            ViewChunkQueueResult::Sent => 1,
            ViewChunkQueueResult::Gone => 2,
        })
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
        .function(wrap_function!(
            cobblestone_session_protocol84_send_prepared_view_chunks
        ))
        .function(wrap_function!(
            cobblestone_session_protocol84_commit_prepared_view
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

    #[test]
    fn view_delta_projection_is_little_endian_and_ordered() {
        let delta = ChunkViewDelta {
            from_center: ChunkCoord::new(-1, 2),
            to_center: ChunkCoord::new(3, -4),
            entering: vec![ChunkCoord::new(5, 6), ChunkCoord::new(-7, 8)],
            leaving: Vec::new(),
        };

        let projection = encode_view_delta(Some(&delta)).expect("encode view delta");
        let mut expected = Vec::new();
        for coordinate in [-1_i32, 2, 3, -4] {
            expected.extend_from_slice(&coordinate.to_le_bytes());
        }
        expected.extend_from_slice(&2_u32.to_le_bytes());
        for coordinate in [5_i32, 6, -7, 8] {
            expected.extend_from_slice(&coordinate.to_le_bytes());
        }

        assert_eq!(projection.as_slice(), expected.as_slice());
        assert!(
            encode_view_delta(None)
                .expect("encode empty view delta")
                .as_slice()
                .is_empty()
        );
    }
}
