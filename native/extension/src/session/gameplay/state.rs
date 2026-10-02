use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, MutexGuard};

use cobblestone_protocol84::MovePlayerPacket;
use cobblestone_runtime::RuntimeId;
use cobblestone_session::SessionId;
use cobblestone_world::ChunkCoord;

use crate::session::view::ChunkViewDelta;

const CHUNK_EDGE: f32 = 16.0;
pub(super) const MAX_PLAYER_COORDINATE: f32 = 1_000_000.0;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PlayerState {
    pub(super) position: [f32; 3],
    pub(super) chunk: ChunkCoord,
    pub(super) desired_radius: Option<i32>,
    pub(super) view_delta: Option<ChunkViewDelta>,
}

static PLAYER_STATES: LazyLock<Mutex<HashMap<(RuntimeId, SessionId), PlayerState>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(super) fn player_states() -> MutexGuard<'static, HashMap<(RuntimeId, SessionId), PlayerState>> {
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
        desired_radius: None,
        view_delta: None,
    })
}

pub(super) fn state_from_move(packet: MovePlayerPacket) -> Result<PlayerState, &'static str> {
    validate_position(packet.position())
}

pub(super) fn spawn_state(spawn: [i32; 3]) -> Result<PlayerState, &'static str> {
    validate_position([
        spawn[0] as f32 + 0.5,
        spawn[1] as f32,
        spawn[2] as f32 + 0.5,
    ])
}

pub(crate) fn forget_session(owner: RuntimeId, session_id: SessionId) {
    player_states().remove(&(owner, session_id));
}

pub(crate) fn forget_runtime(owner: RuntimeId) {
    player_states().retain(|(runtime, _), _| *runtime != owner);
}
