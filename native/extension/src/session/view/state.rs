use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

use cobblestone_runtime::RuntimeId;
use cobblestone_session::SessionId;
use cobblestone_world::{ChunkCoord, WorldStore};
use ext_php_rs::exception::PhpResult;

use crate::boundary::php_error;

use super::geometry::{
    ChunkViewDelta, chunk_view_delta, chunk_view_transition, view_contains, view_positions,
};

#[derive(Debug, Clone)]
pub(super) struct WorldView {
    pub(super) world_handle: i64,
    pub(super) store: Arc<WorldStore>,
    pub(super) center: ChunkCoord,
    pub(super) radius: i32,
    pub(super) cursor: u64,
    pub(super) pinned_chunks: Vec<ChunkCoord>,
}

impl WorldView {
    pub(super) fn contains(&self, position: ChunkCoord) -> bool {
        view_contains(self.center, self.radius, position)
    }
}

static WORLD_VIEWS: LazyLock<Mutex<HashMap<(RuntimeId, SessionId), WorldView>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(super) fn world_views() -> MutexGuard<'static, HashMap<(RuntimeId, SessionId), WorldView>> {
    match WORLD_VIEWS.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub(crate) fn plan_view_delta(
    owner: RuntimeId,
    session_id: SessionId,
    to_center: ChunkCoord,
) -> Option<ChunkViewDelta> {
    let views = world_views();
    let view = views.get(&(owner, session_id))?;
    chunk_view_delta(view.center, view.radius, to_center)
}

pub(crate) fn plan_view_transition(
    owner: RuntimeId,
    session_id: SessionId,
    to_center: ChunkCoord,
    to_radius: i32,
) -> Option<ChunkViewDelta> {
    let views = world_views();
    let view = views.get(&(owner, session_id))?;
    chunk_view_transition(view.center, view.radius, to_center, to_radius)
}

pub(super) fn apply_view_delta(view: &mut WorldView, delta: &ChunkViewDelta) -> Result<(), String> {
    if view.center != delta.from_center || view.radius != delta.from_radius {
        return Err(
            "pending chunk view delta no longer matches the active world view geometry".into(),
        );
    }

    let expected =
        chunk_view_transition(view.center, view.radius, delta.to_center, delta.to_radius)
            .ok_or_else(|| {
                "pending chunk view delta does not change the active view".to_string()
            })?;
    if expected != *delta {
        return Err("pending chunk view delta does not match the active view geometry".into());
    }

    let mut acquired = Vec::with_capacity(delta.entering.len());
    for &position in &delta.entering {
        if let Err(error) = view.store.pin_chunk(position) {
            for &rollback in &acquired {
                let _ = view.store.unpin_chunk(rollback);
            }
            return Err(error.to_string());
        }
        acquired.push(position);
    }

    view.center = delta.to_center;
    view.radius = delta.to_radius;
    view.pinned_chunks = view_positions(delta.to_center, delta.to_radius);

    for &position in &delta.leaving {
        let _ = view.store.unpin_chunk(position);
    }

    Ok(())
}

pub(crate) fn commit_view_delta(
    owner: RuntimeId,
    session_id: SessionId,
    delta: &ChunkViewDelta,
) -> PhpResult<bool> {
    let mut views = world_views();
    let Some(view) = views.get_mut(&(owner, session_id)) else {
        return Ok(false);
    };

    apply_view_delta(view, delta).map_err(php_error)?;
    Ok(true)
}

pub(super) fn release_view(view: &WorldView) {
    for &position in &view.pinned_chunks {
        let _ = view.store.unpin_chunk(position);
    }
}

pub(crate) fn forget_session(owner: RuntimeId, session_id: SessionId) {
    if let Some(view) = world_views().remove(&(owner, session_id)) {
        release_view(&view);
    }
}

pub(crate) fn forget_runtime(owner: RuntimeId) {
    let removed = {
        let mut views = world_views();
        let keys = views
            .keys()
            .filter(|(runtime, _)| *runtime == owner)
            .copied()
            .collect::<Vec<_>>();
        keys.into_iter()
            .filter_map(|key| views.remove(&key))
            .collect::<Vec<_>>()
    };

    for view in removed {
        release_view(&view);
    }
}

pub(crate) fn install_view(
    key: (RuntimeId, SessionId),
    world_handle: i64,
    store: Arc<WorldStore>,
    center: ChunkCoord,
    radius: i32,
    cursor: u64,
    pinned_chunks: Vec<ChunkCoord>,
) {
    let view = WorldView {
        world_handle,
        store,
        center,
        radius,
        cursor,
        pinned_chunks,
    };
    if let Some(previous) = world_views().insert(key, view) {
        release_view(&previous);
    }
}
