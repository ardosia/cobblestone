use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

use cobblestone_codec::{BatchPacket, BootstrapPacket, RawPacket};
use cobblestone_core::{ChunkCoord, NativeBuffer, RuntimeId, WorldStore};
use cobblestone_session::{SessionDelivery, SessionId};
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::php_error;
use crate::session::bridge::{QueueResult, try_queue};
use crate::world::protocol84_chunk;

use super::join::{CHUNK_RADIUS_UPDATED_ID, bootstrap_session_packet};

mod sync;

#[derive(Debug, Clone)]
struct WorldView {
    world_handle: i64,
    store: Arc<WorldStore>,
    center: ChunkCoord,
    radius: i32,
    cursor: u64,
    pinned_chunks: Vec<ChunkCoord>,
}

fn view_contains(center: ChunkCoord, radius: i32, position: ChunkCoord) -> bool {
    position.x() >= center.x().saturating_sub(radius)
        && position.x() <= center.x().saturating_add(radius)
        && position.z() >= center.z().saturating_sub(radius)
        && position.z() <= center.z().saturating_add(radius)
}

fn view_positions(center: ChunkCoord, radius: i32) -> Vec<ChunkCoord> {
    debug_assert!(radius >= 0);
    let min_x = center.x().saturating_sub(radius);
    let max_x = center.x().saturating_add(radius);
    let min_z = center.z().saturating_sub(radius);
    let max_z = center.z().saturating_add(radius);
    let side = usize::try_from(radius.saturating_mul(2).saturating_add(1)).unwrap_or(0);
    let mut positions = Vec::with_capacity(side.saturating_mul(side));

    for x in min_x..=max_x {
        for z in min_z..=max_z {
            positions.push(ChunkCoord::new(x, z));
        }
    }

    positions
}

impl WorldView {
    fn contains(&self, position: ChunkCoord) -> bool {
        view_contains(self.center, self.radius, position)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChunkViewDelta {
    pub(crate) from_center: ChunkCoord,
    pub(crate) to_center: ChunkCoord,
    pub(crate) from_radius: i32,
    pub(crate) to_radius: i32,
    pub(crate) entering: Vec<ChunkCoord>,
    pub(crate) leaving: Vec<ChunkCoord>,
}

fn chunk_view_transition(
    from_center: ChunkCoord,
    from_radius: i32,
    to_center: ChunkCoord,
    to_radius: i32,
) -> Option<ChunkViewDelta> {
    if from_center == to_center && from_radius == to_radius {
        return None;
    }

    let entering = view_positions(to_center, to_radius)
        .into_iter()
        .filter(|&position| !view_contains(from_center, from_radius, position))
        .collect();
    let leaving = view_positions(from_center, from_radius)
        .into_iter()
        .filter(|&position| !view_contains(to_center, to_radius, position))
        .collect();

    Some(ChunkViewDelta {
        from_center,
        to_center,
        from_radius,
        to_radius,
        entering,
        leaving,
    })
}

fn chunk_view_delta(
    from_center: ChunkCoord,
    radius: i32,
    to_center: ChunkCoord,
) -> Option<ChunkViewDelta> {
    chunk_view_transition(from_center, radius, to_center, radius)
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

fn apply_view_delta(view: &mut WorldView, delta: &ChunkViewDelta) -> Result<(), String> {
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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum ViewChunkQueueResult {
    Sent,
    Backpressured,
    Gone,
}

pub(crate) fn queue_view_delta_chunks(
    owner: RuntimeId,
    session_id: SessionId,
    delta: &ChunkViewDelta,
) -> PhpResult<ViewChunkQueueResult> {
    let (world_handle, center, radius) = {
        let views = world_views();
        let view = views
            .get(&(owner, session_id))
            .ok_or_else(|| php_error("cannot send entering chunks without an active world view"))?;
        (view.world_handle, view.center, view.radius)
    };

    if center != delta.from_center || radius != delta.from_radius {
        return Err(php_error(
            "pending chunk view delta no longer matches the active world view geometry",
        ));
    }

    let radius_changed = delta.from_radius != delta.to_radius;
    let mut packets = Vec::with_capacity(delta.entering.len() + usize::from(radius_changed));
    if radius_changed {
        packets.push(RawPacket::new(
            CHUNK_RADIUS_UPDATED_ID,
            NativeBuffer::copy_from_slice(&delta.to_radius.to_be_bytes()),
        ));
    }
    for &position in &delta.entering {
        packets.push(protocol84_chunk(world_handle, position)?);
    }
    if packets.is_empty() {
        return Ok(ViewChunkQueueResult::Sent);
    }
    let batch = bootstrap_session_packet(BootstrapPacket::Batch(BatchPacket::new(packets)))?;

    Ok(
        match try_queue(owner, session_id, batch, SessionDelivery::ReliableOrdered)? {
            QueueResult::Sent => ViewChunkQueueResult::Sent,
            QueueResult::Backpressured => ViewChunkQueueResult::Backpressured,
            QueueResult::Gone => ViewChunkQueueResult::Gone,
        },
    )
}

static WORLD_VIEWS: LazyLock<Mutex<HashMap<(RuntimeId, SessionId), WorldView>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn world_views() -> MutexGuard<'static, HashMap<(RuntimeId, SessionId), WorldView>> {
    match WORLD_VIEWS.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn release_view(view: &WorldView) {
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

pub(crate) fn register(module: ModuleBuilder) -> ModuleBuilder {
    sync::register(module)
}

#[cfg(test)]
mod view_delta_tests {
    use super::*;
    use cobblestone_core::{ChunkPatch, WORLD_CHANGE_LOG_CAPACITY};

    fn positions(values: &[(i32, i32)]) -> Vec<ChunkCoord> {
        values.iter().map(|&(x, z)| ChunkCoord::new(x, z)).collect()
    }

    fn pinned_view(center: ChunkCoord, radius: i32) -> WorldView {
        let store = Arc::new(WorldStore::new());
        let pinned_chunks = view_positions(center, radius);
        for &position in &pinned_chunks {
            store.ensure_chunk(position, 1);
            store.pin_chunk(position).expect("pin initial view chunk");
        }

        WorldView {
            world_handle: 1,
            store,
            center,
            radius,
            cursor: 7,
            pinned_chunks,
        }
    }

    fn record_block_change(
        store: &WorldStore,
        position: ChunkCoord,
        terrain_revision: u64,
        state: u16,
    ) {
        store
            .apply_patch(
                position,
                ChunkPatch {
                    expected_terrain_revision: terrain_revision,
                    next_terrain_revision: terrain_revision + 1,
                    expected_light_revision: 0,
                    next_light_revision: 0,
                    blocks: vec![(0, state)],
                    biomes: Vec::new(),
                    extra_data: Vec::new(),
                    sky_light: Vec::new(),
                    block_light: Vec::new(),
                },
            )
            .expect("record block change");
    }

    #[test]
    fn unchanged_center_has_no_delta() {
        assert_eq!(
            chunk_view_delta(ChunkCoord::new(8, 8), 2, ChunkCoord::new(8, 8)),
            None
        );
    }

    #[test]
    fn cardinal_shift_has_deterministic_entering_and_leaving_edges() {
        let delta =
            chunk_view_delta(ChunkCoord::new(0, 0), 1, ChunkCoord::new(1, 0)).expect("delta");

        assert_eq!(delta.from_center, ChunkCoord::new(0, 0));
        assert_eq!(delta.to_center, ChunkCoord::new(1, 0));
        assert_eq!(delta.entering, positions(&[(2, -1), (2, 0), (2, 1)]));
        assert_eq!(delta.leaving, positions(&[(-1, -1), (-1, 0), (-1, 1)]));
    }

    #[test]
    fn diagonal_shift_keeps_x_then_z_order() {
        let delta =
            chunk_view_delta(ChunkCoord::new(0, 0), 1, ChunkCoord::new(1, 1)).expect("delta");

        assert_eq!(
            delta.entering,
            positions(&[(0, 2), (1, 2), (2, 0), (2, 1), (2, 2)])
        );
        assert_eq!(
            delta.leaving,
            positions(&[(-1, -1), (-1, 0), (-1, 1), (0, -1), (1, -1)])
        );
    }

    #[test]
    fn multi_chunk_jump_replaces_the_entire_disjoint_view() {
        let delta =
            chunk_view_delta(ChunkCoord::new(0, 0), 1, ChunkCoord::new(4, 0)).expect("delta");

        assert_eq!(delta.entering.len(), 9);
        assert_eq!(delta.leaving.len(), 9);
        assert_eq!(delta.entering.first(), Some(&ChunkCoord::new(3, -1)));
        assert_eq!(delta.entering.last(), Some(&ChunkCoord::new(5, 1)));
        assert_eq!(delta.leaving.first(), Some(&ChunkCoord::new(-1, -1)));
        assert_eq!(delta.leaving.last(), Some(&ChunkCoord::new(1, 1)));
        assert!(
            delta
                .entering
                .iter()
                .all(|position| !delta.leaving.contains(position))
        );
    }

    #[test]
    fn radius_grow_and_shrink_have_inverse_edges() {
        let grow = chunk_view_transition(ChunkCoord::new(0, 0), 1, ChunkCoord::new(0, 0), 2)
            .expect("grow delta");
        assert_eq!(grow.from_radius, 1);
        assert_eq!(grow.to_radius, 2);
        assert_eq!(grow.entering.len(), 16);
        assert!(grow.leaving.is_empty());

        let shrink = chunk_view_transition(ChunkCoord::new(0, 0), 2, ChunkCoord::new(0, 0), 1)
            .expect("shrink delta");
        assert!(shrink.entering.is_empty());
        assert_eq!(shrink.leaving, grow.entering);
    }

    #[test]
    fn applying_delta_transfers_native_pins_and_preserves_cursor() {
        let mut view = pinned_view(ChunkCoord::new(0, 0), 1);
        let delta =
            chunk_view_delta(view.center, view.radius, ChunkCoord::new(1, 0)).expect("delta");
        for &position in &delta.entering {
            view.store.ensure_chunk(position, 1);
        }

        let shared_leaving = delta.leaving[0];
        assert_eq!(view.store.pin_chunk(shared_leaving).unwrap(), 2);

        apply_view_delta(&mut view, &delta).expect("apply view delta");

        assert_eq!(view.center, ChunkCoord::new(1, 0));
        assert_eq!(view.cursor, 7);
        assert_eq!(view.pinned_chunks, view_positions(ChunkCoord::new(1, 0), 1));
        for &position in &delta.entering {
            assert_eq!(view.store.pin_count(position).unwrap(), 1);
        }
        for &position in &delta.leaving[1..] {
            assert_eq!(view.store.pin_count(position).unwrap(), 0);
        }
        assert_eq!(view.store.pin_count(shared_leaving).unwrap(), 1);
        assert_eq!(view.store.pin_count(ChunkCoord::new(0, 0)).unwrap(), 1);
    }

    #[test]
    fn failed_entry_pin_rolls_back_without_publishing_new_view() {
        let mut view = pinned_view(ChunkCoord::new(0, 0), 1);
        let original_positions = view.pinned_chunks.clone();
        let delta =
            chunk_view_delta(view.center, view.radius, ChunkCoord::new(1, 0)).expect("delta");

        for &position in &delta.entering[..2] {
            view.store.ensure_chunk(position, 1);
        }

        assert!(apply_view_delta(&mut view, &delta).is_err());
        assert_eq!(view.center, ChunkCoord::new(0, 0));
        assert_eq!(view.pinned_chunks, original_positions);
        for &position in &delta.entering[..2] {
            assert_eq!(view.store.pin_count(position).unwrap(), 0);
        }
        for &position in &delta.leaving {
            assert_eq!(view.store.pin_count(position).unwrap(), 1);
        }
    }

    #[test]
    fn recentered_view_filters_changes_using_preserved_cursor() {
        let mut view = pinned_view(ChunkCoord::new(0, 0), 1);
        view.cursor = view.store.current_change_sequence();
        let delta =
            chunk_view_delta(view.center, view.radius, ChunkCoord::new(1, 0)).expect("delta");
        for &position in &delta.entering {
            view.store.ensure_chunk(position, 1);
        }

        apply_view_delta(&mut view, &delta).expect("apply view delta");
        assert_eq!(view.cursor, 0);

        let old_only = ChunkCoord::new(-1, 0);
        let current_only = ChunkCoord::new(2, 0);
        record_block_change(&view.store, old_only, 0, 1);
        record_block_change(&view.store, current_only, 0, 1);

        let log = view.store.change_log_snapshot();
        let pending = sync::pending_view_changes(&view, &log);

        assert_eq!(pending.len(), 1);
        assert!(!pending.contains_key(&old_only));
        assert!(matches!(
            pending.get(&current_only),
            Some(sync::PendingChunkSync::Blocks(_))
        ));
    }

    #[test]
    fn stale_cursor_after_recenter_recovers_exact_current_view() {
        let mut view = pinned_view(ChunkCoord::new(0, 0), 1);
        view.cursor = view.store.current_change_sequence();
        let delta =
            chunk_view_delta(view.center, view.radius, ChunkCoord::new(1, 0)).expect("delta");
        for &position in &delta.entering {
            view.store.ensure_chunk(position, 1);
        }
        apply_view_delta(&mut view, &delta).expect("apply view delta");

        let current = ChunkCoord::new(1, 0);
        for revision in 0..=WORLD_CHANGE_LOG_CAPACITY as u64 {
            record_block_change(
                &view.store,
                current,
                revision,
                u16::try_from(revision & 1).expect("binary state"),
            );
        }

        let log = view.store.change_log_snapshot();
        assert!(log.cursor_is_stale(view.cursor));

        let pending = sync::pending_view_changes(&view, &log);
        assert_eq!(pending.len(), view.pinned_chunks.len());
        assert!(
            pending
                .values()
                .all(|change| *change == sync::PendingChunkSync::FullChunk)
        );

        let mut recovered = pending.into_keys().collect::<Vec<_>>();
        recovered.sort_unstable_by_key(|position| (position.x(), position.z()));
        assert_eq!(recovered, view_positions(ChunkCoord::new(1, 0), 1));
        assert!(!recovered.contains(&ChunkCoord::new(-1, 0)));
    }
}
