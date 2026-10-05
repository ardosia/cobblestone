use cobblestone_protocol84::{BatchPacket, BootstrapPacket, RawPacket};
use cobblestone_runtime::{NativeBuffer, RuntimeId};
use cobblestone_session::{SessionDelivery, SessionId};
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::php_error;
use crate::session::bridge::{QueueResult, try_queue};
use crate::world::protocol84_chunk;

use super::join::{CHUNK_RADIUS_UPDATED_ID, bootstrap_session_packet};

mod geometry;
mod state;
mod sync;

pub(crate) use geometry::{ChunkViewDelta, prioritize_for_player};
use state::world_views;

pub(crate) use state::{
    commit_view_delta, forget_runtime, forget_session, install_view, plan_view_delta,
    plan_view_transition,
};

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

pub(crate) fn register(module: ModuleBuilder) -> ModuleBuilder {
    sync::register(module)
}

#[cfg(test)]
mod view_delta_tests {
    use super::geometry::{
        chunk_view_delta, chunk_view_transition, prioritize_for_player, view_positions,
    };
    use std::sync::Arc;

    use super::state::{WorldView, apply_view_delta};
    use super::*;
    use cobblestone_world::{ChunkCoord, ChunkPatch, WORLD_CHANGE_LOG_CAPACITY, WorldStore};

    fn positions(values: &[(i32, i32)]) -> Vec<ChunkCoord> {
        values.iter().map(|&(x, z)| ChunkCoord::new(x, z)).collect()
    }

    fn pinned_view(center: ChunkCoord, radius: i32) -> WorldView {
        let store = Arc::new(WorldStore::new());
        let pinned_chunks = view_positions(center, radius);
        for &position in &pinned_chunks {
            store.ensure_chunk(position, 1).unwrap();
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
    fn diagonal_shift_keeps_target_x_fast_grid_order() {
        let delta =
            chunk_view_delta(ChunkCoord::new(0, 0), 1, ChunkCoord::new(1, 1)).expect("delta");

        assert_eq!(
            delta.entering,
            positions(&[(2, 0), (2, 1), (0, 2), (1, 2), (2, 2)])
        );
        assert_eq!(
            delta.leaving,
            positions(&[(-1, -1), (0, -1), (1, -1), (-1, 0), (-1, 1)])
        );
    }

    #[test]
    fn target_streaming_priority_prefers_chunk_min_nearest_player() {
        let mut entering = positions(&[(0, 9), (1, 9), (2, 9), (0, 10), (1, 10)]);
        prioritize_for_player(&mut entering, [20.0, 64.0, 148.0]);

        assert_eq!(
            entering,
            positions(&[(1, 9), (2, 9), (1, 10), (0, 9), (0, 10)])
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
            view.store.ensure_chunk(position, 1).unwrap();
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
    fn target_priority_reordering_still_commits_the_same_view_geometry() {
        let mut view = pinned_view(ChunkCoord::new(0, 0), 1);
        let mut delta =
            chunk_view_delta(view.center, view.radius, ChunkCoord::new(1, 0)).expect("delta");
        prioritize_for_player(&mut delta.entering, [24.5, 64.0, 0.5]);
        assert_eq!(
            delta.entering,
            positions(&[(2, 0), (2, 1), (2, -1)]),
            "fixture must exercise an order different from raw GridArea order",
        );
        for &position in &delta.entering {
            view.store.ensure_chunk(position, 1).unwrap();
        }

        apply_view_delta(&mut view, &delta).expect("priority-reordered view delta");
        assert_eq!(view.center, ChunkCoord::new(1, 0));
        assert_eq!(view.pinned_chunks, view_positions(ChunkCoord::new(1, 0), 1));
    }

    #[test]
    fn failed_entry_pin_rolls_back_without_publishing_new_view() {
        let mut view = pinned_view(ChunkCoord::new(0, 0), 1);
        let original_positions = view.pinned_chunks.clone();
        let delta =
            chunk_view_delta(view.center, view.radius, ChunkCoord::new(1, 0)).expect("delta");

        for &position in &delta.entering[..2] {
            view.store.ensure_chunk(position, 1).unwrap();
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
            view.store.ensure_chunk(position, 1).unwrap();
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
            view.store.ensure_chunk(position, 1).unwrap();
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
        recovered.sort_unstable_by_key(|position| (position.z(), position.x()));
        assert_eq!(recovered, view_positions(ChunkCoord::new(1, 0), 1));
        assert!(!recovered.contains(&ChunkCoord::new(-1, 0)));
    }
}
