mod projection;
mod state;

use cobblestone_protocol84::decode_protocol84_move_player;
use cobblestone_world::ChunkCoord;
use ext_php_rs::binary::Binary;
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::{php_boundary, php_error};
use crate::runtime::current_runtime_id;
use crate::session::bridge::owner_session_id;
use crate::session::view::{
    ViewChunkQueueResult, commit_view_delta, plan_view_delta, plan_view_transition,
    queue_view_delta_chunks,
};

use projection::encode_view_delta;
#[cfg(test)]
use state::{MAX_PLAYER_COORDINATE, PlayerState};
pub(crate) use state::{forget_runtime, forget_session};
use state::{player_states, spawn_state, state_from_move};

const MAX_VIEW_RADIUS: i32 = 3;

fn i32_field(field: &'static str, value: i64) -> PhpResult<i32> {
    i32::try_from(value).map_err(|_| php_error(format!("{field} must fit signed 32-bit range")))
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

        let desired_radius = {
            let states = player_states();
            states
                .get(&(owner, session_id))
                .ok_or_else(|| {
                    php_error("protocol-84 MovePlayer received before spawned player state")
                })?
                .desired_radius
        };
        state.desired_radius = desired_radius;
        state.view_delta = match desired_radius {
            Some(radius) => plan_view_transition(owner, session_id, state.chunk, radius),
            None => plan_view_delta(owner, session_id, state.chunk),
        };
        let projection = encode_view_delta(state.view_delta.as_ref()).map_err(php_error)?;
        player_states().insert((owner, session_id), state);
        Ok(projection)
    })
}

/// Plans one post-spawn effective-radius change around the latest player chunk.
#[php_function]
#[php(name = "cobblestone_session_protocol84_plan_chunk_radius")]
pub fn cobblestone_session_protocol84_plan_chunk_radius(
    session_id: i64,
    effective_radius: i64,
) -> PhpResult<Binary<u8>> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let session_id = owner_session_id(session_id)?;
        let effective_radius = i32_field("effective chunk radius", effective_radius)?;
        if !(1..=MAX_VIEW_RADIUS).contains(&effective_radius) {
            return Err(php_error(format!(
                "effective post-spawn chunk radius must be in range 1..={MAX_VIEW_RADIUS}"
            )));
        }

        let chunk = {
            let states = player_states();
            states
                .get(&(owner, session_id))
                .ok_or_else(|| php_error("cannot resize chunk view before spawned player state"))?
                .chunk
        };
        let delta = plan_view_transition(owner, session_id, chunk, effective_radius);
        let projection = encode_view_delta(delta.as_ref()).map_err(php_error)?;

        let mut states = player_states();
        let state = states
            .get_mut(&(owner, session_id))
            .ok_or_else(|| php_error("spawned player state disappeared during view resize"))?;
        state.desired_radius = Some(effective_radius);
        state.view_delta = delta;
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
    from_radius: i64,
    to_chunk_x: i64,
    to_chunk_z: i64,
    to_radius: i64,
) -> PhpResult<bool> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let session_id = owner_session_id(session_id)?;
        let from_center = ChunkCoord::new(
            i32_field("view transition from chunk x", from_chunk_x)?,
            i32_field("view transition from chunk z", from_chunk_z)?,
        );
        let from_radius = i32_field("view transition from radius", from_radius)?;
        let to_center = ChunkCoord::new(
            i32_field("view transition to chunk x", to_chunk_x)?,
            i32_field("view transition to chunk z", to_chunk_z)?,
        );
        let to_radius = i32_field("view transition to radius", to_radius)?;

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

        if delta.from_center != from_center
            || delta.from_radius != from_radius
            || delta.to_center != to_center
            || delta.to_radius != to_radius
        {
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
    from_radius: i64,
    to_chunk_x: i64,
    to_chunk_z: i64,
    to_radius: i64,
) -> PhpResult<i64> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let session_id = owner_session_id(session_id)?;
        let from_center = ChunkCoord::new(
            i32_field("view transition from chunk x", from_chunk_x)?,
            i32_field("view transition from chunk z", from_chunk_z)?,
        );
        let from_radius = i32_field("view transition from radius", from_radius)?;
        let to_center = ChunkCoord::new(
            i32_field("view transition to chunk x", to_chunk_x)?,
            i32_field("view transition to chunk z", to_chunk_z)?,
        );
        let to_radius = i32_field("view transition to radius", to_radius)?;

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

        if delta.from_center != from_center
            || delta.from_radius != from_radius
            || delta.to_center != to_center
            || delta.to_radius != to_radius
        {
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
            cobblestone_session_protocol84_plan_chunk_radius
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
}
