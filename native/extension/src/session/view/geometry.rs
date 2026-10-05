use cobblestone_world::ChunkCoord;

pub(super) fn view_contains(center: ChunkCoord, radius: i32, position: ChunkCoord) -> bool {
    position.x() >= center.x().saturating_sub(radius)
        && position.x() <= center.x().saturating_add(radius)
        && position.z() >= center.z().saturating_sub(radius)
        && position.z() <= center.z().saturating_add(radius)
}

pub(super) fn view_positions(center: ChunkCoord, radius: i32) -> Vec<ChunkCoord> {
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

/// Serializes the target streaming worker's chunk priority into a deterministic request order.
///
/// MCPE 0.15.10 queues chunk loads at _getChunkPriority(chunk) = floor(distance(chunk.min,
/// player)) + time*16. The common time term does not affect ordering inside one view update.
/// Its worker consumes the smallest priority first. Stable sorting preserves the target GridArea
/// insertion order for equal integer-distance priorities.
pub(crate) fn prioritize_for_player(positions: &mut [ChunkCoord], player: [f32; 3]) {
    positions.sort_by_key(|position| streaming_distance_priority(*position, player));
}

fn streaming_distance_priority(position: ChunkCoord, player: [f32; 3]) -> i32 {
    let chunk_x = position.x() as f32 * 16.0;
    let chunk_z = position.z() as f32 * 16.0;
    let dx = chunk_x - player[0];
    let dz = chunk_z - player[2];
    dx.mul_add(dx, dz * dz).sqrt() as i32
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

pub(super) fn chunk_view_transition(
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

pub(super) fn chunk_view_delta(
    from_center: ChunkCoord,
    radius: i32,
    to_center: ChunkCoord,
) -> Option<ChunkViewDelta> {
    chunk_view_transition(from_center, radius, to_center, radius)
}
