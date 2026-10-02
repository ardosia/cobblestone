use cobblestone_core::ChunkCoord;

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

    for x in min_x..=max_x {
        for z in min_z..=max_z {
            positions.push(ChunkCoord::new(x, z));
        }
    }

    positions
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
