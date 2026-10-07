use std::collections::HashSet;
use std::sync::Arc;

use cobblestone_target::ChunkShape;
use cobblestone_world::{ChunkCoord, WorldStore};

pub(crate) fn view_contains(center: ChunkCoord, radius: i32, position: ChunkCoord) -> bool {
    position.x() >= center.x().saturating_sub(radius)
        && position.x() <= center.x().saturating_add(radius)
        && position.z() >= center.z().saturating_sub(radius)
        && position.z() <= center.z().saturating_add(radius)
}

pub(crate) fn view_positions(center: ChunkCoord, radius: i32) -> Vec<ChunkCoord> {
    debug_assert!(radius >= 0);
    let min_x = center.x().saturating_sub(radius);
    let max_x = center.x().saturating_add(radius);
    let min_z = center.z().saturating_sub(radius);
    let max_z = center.z().saturating_add(radius);
    let side = usize::try_from(radius.saturating_mul(2).saturating_add(1)).unwrap_or(0);
    let mut positions = Vec::with_capacity(side.saturating_mul(side));

    // MCPE GridArea/Bounds iteration advances X first, then Z.
    for z in min_z..=max_z {
        for x in min_x..=max_x {
            positions.push(ChunkCoord::new(x, z));
        }
    }

    positions
}

pub(crate) fn initial_send_positions(center: ChunkCoord, radius: i32) -> Vec<ChunkCoord> {
    debug_assert!(radius >= 0);
    let min_x = center.x().saturating_sub(radius);
    let max_x = center.x().saturating_add(radius);
    let min_z = center.z().saturating_sub(radius);
    let max_z = center.z().saturating_add(radius);
    let side = usize::try_from(radius.saturating_mul(2).saturating_add(1)).unwrap_or(0);
    let mut positions = Vec::with_capacity(side.saturating_mul(side));

    // Preserve the existing full-view encoding order used by the native initial-chunk bridge.
    for x in min_x..=max_x {
        for z in min_z..=max_z {
            positions.push(ChunkCoord::new(x, z));
        }
    }

    positions
}

/// Serializes the target streaming worker's chunk priority into a deterministic request order.
pub(crate) fn prioritize_for_player(positions: &mut [ChunkCoord], player: [f32; 3]) {
    positions.sort_by_key(|position| streaming_distance_priority(*position, player));
}

fn streaming_distance_priority(position: ChunkCoord, player: [f32; 3]) -> i32 {
    let chunk_x = position.x() as f32 * ChunkShape::EDGE as f32;
    let chunk_z = position.z() as f32 * ChunkShape::EDGE as f32;
    let dx = chunk_x - player[0];
    let dz = chunk_z - player[2];
    dx.mul_add(dx, dz * dz).sqrt() as i32
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChunkViewTransition {
    pub(crate) from_center: ChunkCoord,
    pub(crate) to_center: ChunkCoord,
    pub(crate) from_radius: i32,
    pub(crate) to_radius: i32,
    pub(crate) entering: Vec<ChunkCoord>,
    pub(crate) leaving: Vec<ChunkCoord>,
}

pub(crate) fn chunk_view_transition(
    from_center: ChunkCoord,
    from_radius: i32,
    to_center: ChunkCoord,
    to_radius: i32,
) -> Option<ChunkViewTransition> {
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

    Some(ChunkViewTransition {
        from_center,
        to_center,
        from_radius,
        to_radius,
        entering,
        leaving,
    })
}

pub(crate) struct PreparedChunks {
    store: Option<Arc<WorldStore>>,
    pinned: HashSet<ChunkCoord>,
}

impl PreparedChunks {
    pub(crate) fn new() -> Self {
        Self {
            store: None,
            pinned: HashSet::new(),
        }
    }

    pub(crate) fn mark(
        &mut self,
        expected: &[ChunkCoord],
        store: Arc<WorldStore>,
        position: ChunkCoord,
    ) -> Result<(), String> {
        if !expected.contains(&position) {
            return Err(format!(
                "chunk {}:{} is not part of the pending session view",
                position.x(),
                position.z()
            ));
        }
        if self.pinned.contains(&position) {
            return Ok(());
        }
        if let Some(current) = &self.store
            && !Arc::ptr_eq(current, &store)
        {
            return Err("pending session view spans multiple world stores".into());
        }

        store
            .pin_chunk(position)
            .map_err(|error| error.to_string())?;
        self.store.get_or_insert_with(|| Arc::clone(&store));
        self.pinned.insert(position);
        Ok(())
    }

    pub(crate) fn unprepared(&self, expected: &[ChunkCoord]) -> Vec<ChunkCoord> {
        expected
            .iter()
            .copied()
            .filter(|position| !self.pinned.contains(position))
            .collect()
    }

    pub(crate) fn is_complete(&self, expected: &[ChunkCoord]) -> bool {
        self.pinned.len() == expected.len() && self.unprepared(expected).is_empty()
    }

    pub(crate) fn matches_store(&self, store: &Arc<WorldStore>) -> bool {
        self.store
            .as_ref()
            .is_none_or(|current| Arc::ptr_eq(current, store))
    }

    pub(crate) fn take_store(&mut self) -> Option<Arc<WorldStore>> {
        self.store.take()
    }

    pub(crate) fn disarm(&mut self) {
        self.pinned.clear();
        self.store = None;
    }
}

impl Drop for PreparedChunks {
    fn drop(&mut self) {
        let Some(store) = self.store.as_ref() else {
            return;
        };
        for &position in &self.pinned {
            let _ = store.unpin_chunk(position);
        }
    }
}

pub(crate) struct WorldView {
    pub(crate) store: Arc<WorldStore>,
    pub(crate) center: ChunkCoord,
    pub(crate) radius: i32,
    pub(crate) cursor: u64,
    pub(crate) pinned_chunks: Vec<ChunkCoord>,
}

impl Drop for WorldView {
    fn drop(&mut self) {
        for &position in &self.pinned_chunks {
            let _ = self.store.unpin_chunk(position);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn positions(values: &[(i32, i32)]) -> Vec<ChunkCoord> {
        values.iter().map(|&(x, z)| ChunkCoord::new(x, z)).collect()
    }

    #[test]
    fn cardinal_shift_has_deterministic_entering_and_leaving_edges() {
        let delta = chunk_view_transition(ChunkCoord::new(0, 0), 1, ChunkCoord::new(1, 0), 1)
            .expect("delta");
        assert_eq!(delta.entering, positions(&[(2, -1), (2, 0), (2, 1)]));
        assert_eq!(delta.leaving, positions(&[(-1, -1), (-1, 0), (-1, 1)]));
    }

    #[test]
    fn radius_grow_and_shrink_have_inverse_edges() {
        let grow = chunk_view_transition(ChunkCoord::new(0, 0), 1, ChunkCoord::new(0, 0), 2)
            .expect("grow");
        let shrink = chunk_view_transition(ChunkCoord::new(0, 0), 2, ChunkCoord::new(0, 0), 1)
            .expect("shrink");
        assert!(grow.leaving.is_empty());
        assert!(shrink.entering.is_empty());
        assert_eq!(shrink.leaving, grow.entering);
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
    fn prepared_chunk_drop_releases_temporary_pins() {
        let store = Arc::new(WorldStore::new());
        let position = ChunkCoord::new(4, -3);
        store.ensure_chunk(position, 1).expect("resident chunk");
        {
            let mut prepared = PreparedChunks::new();
            prepared
                .mark(&[position], Arc::clone(&store), position)
                .expect("temporary pin");
            assert_eq!(store.pin_count(position).expect("pin count"), 1);
        }
        assert_eq!(store.pin_count(position).expect("pin count"), 0);
    }
}
