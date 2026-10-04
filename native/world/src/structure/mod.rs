mod village;
mod village_plan;

use crate::ChunkCoord;
use crate::terrain_shape::noise::MtRandom;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(i8)]
pub enum StructureFeatureKind {
    EndCity = 0,
    Fortress = 1,
    Mineshaft = 2,
    Monument = 3,
    Stronghold = 4,
    Temple = 5,
    Village = 6,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum StructureOrientation {
    South = 0,
    West = 1,
    North = 2,
    East = 3,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct StructureBounds {
    pub(crate) x0: i32,
    pub(crate) y0: i32,
    pub(crate) z0: i32,
    pub(crate) x1: i32,
    pub(crate) y1: i32,
    pub(crate) z1: i32,
}

impl StructureBounds {
    pub(crate) const fn new(x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32) -> Self {
        Self {
            x0,
            y0,
            z0,
            x1,
            y1,
            z1,
        }
    }

    pub(crate) fn unknown() -> Self {
        Self::new(
            i32::MAX,
            i32::MAX,
            i32::MAX,
            -i32::MAX,
            -i32::MAX,
            -i32::MAX,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn orient_box(
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        off_x: i32,
        off_y: i32,
        off_z: i32,
        width: i32,
        height: i32,
        depth: i32,
        orientation: StructureOrientation,
    ) -> Self {
        match orientation {
            StructureOrientation::North => Self::new(
                foot_x.wrapping_add(off_x),
                foot_y.wrapping_add(off_y),
                foot_z
                    .wrapping_sub(depth)
                    .wrapping_add(1)
                    .wrapping_add(off_z),
                foot_x
                    .wrapping_add(width)
                    .wrapping_sub(1)
                    .wrapping_add(off_x),
                foot_y
                    .wrapping_add(height)
                    .wrapping_sub(1)
                    .wrapping_add(off_y),
                foot_z.wrapping_add(off_z),
            ),
            StructureOrientation::South => Self::new(
                foot_x.wrapping_add(off_x),
                foot_y.wrapping_add(off_y),
                foot_z.wrapping_add(off_z),
                foot_x
                    .wrapping_add(width)
                    .wrapping_sub(1)
                    .wrapping_add(off_x),
                foot_y
                    .wrapping_add(height)
                    .wrapping_sub(1)
                    .wrapping_add(off_y),
                foot_z
                    .wrapping_add(depth)
                    .wrapping_sub(1)
                    .wrapping_add(off_z),
            ),
            StructureOrientation::West => Self::new(
                foot_x
                    .wrapping_sub(depth)
                    .wrapping_add(1)
                    .wrapping_add(off_z),
                foot_y.wrapping_add(off_y),
                foot_z.wrapping_add(off_x),
                foot_x.wrapping_add(off_z),
                foot_y
                    .wrapping_add(height)
                    .wrapping_sub(1)
                    .wrapping_add(off_y),
                foot_z
                    .wrapping_add(width)
                    .wrapping_sub(1)
                    .wrapping_add(off_x),
            ),
            StructureOrientation::East => Self::new(
                foot_x.wrapping_add(off_z),
                foot_y.wrapping_add(off_y),
                foot_z.wrapping_add(off_x),
                foot_x
                    .wrapping_add(depth)
                    .wrapping_sub(1)
                    .wrapping_add(off_z),
                foot_y
                    .wrapping_add(height)
                    .wrapping_sub(1)
                    .wrapping_add(off_y),
                foot_z
                    .wrapping_add(width)
                    .wrapping_sub(1)
                    .wrapping_add(off_x),
            ),
        }
    }

    pub(crate) fn intersects(&self, other: Self) -> bool {
        !(self.x1 < other.x0
            || self.x0 > other.x1
            || self.z1 < other.z0
            || self.z0 > other.z1
            || self.y1 < other.y0
            || self.y0 > other.y1)
    }

    pub(crate) fn intersects_xz(&self, x0: i32, z0: i32, x1: i32, z1: i32) -> bool {
        !(self.x1 < x0 || self.x0 > x1 || self.z1 < z0 || self.z0 > z1)
    }

    pub(crate) fn contains(&self, x: i32, y: i32, z: i32) -> bool {
        x >= self.x0 && x <= self.x1 && y >= self.y0 && y <= self.y1 && z >= self.z0 && z <= self.z1
    }

    pub(crate) fn expand(&mut self, other: Self) {
        self.x0 = self.x0.min(other.x0);
        self.y0 = self.y0.min(other.y0);
        self.z0 = self.z0.min(other.z0);
        self.x1 = self.x1.max(other.x1);
        self.y1 = self.y1.max(other.y1);
        self.z1 = self.z1.max(other.z1);
    }

    pub(crate) const fn x_span(&self) -> i32 {
        self.x1 - self.x0 + 1
    }

    pub(crate) const fn y_span(&self) -> i32 {
        self.y1 - self.y0 + 1
    }

    pub(crate) const fn z_span(&self) -> i32 {
        self.z1 - self.z0 + 1
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct StructureStartCore {
    source: ChunkCoord,
    bounds: StructureBounds,
    generated_chunk_positions: Vec<i32>,
}

impl StructureStartCore {
    pub(crate) fn new(source: ChunkCoord, bounds: StructureBounds) -> Self {
        Self {
            source,
            bounds,
            generated_chunk_positions: Vec::new(),
        }
    }

    pub(crate) const fn source(&self) -> ChunkCoord {
        self.source
    }

    pub(crate) const fn bounds(&self) -> StructureBounds {
        self.bounds
    }

    pub(crate) fn should_post_process(&self, target: ChunkCoord) -> bool {
        let cx = target.x().wrapping_mul(16);
        let cz = target.z().wrapping_mul(16);
        self.bounds
            .intersects_xz(cx, cz, cx.wrapping_add(16), cz.wrapping_add(16))
            && !self
                .generated_chunk_positions
                .contains(&chunk_hash(target.x(), target.z()))
    }

    pub(crate) fn mark_post_processed(&mut self, target: ChunkCoord) {
        let hash = chunk_hash(target.x(), target.z());
        if !self.generated_chunk_positions.contains(&hash) {
            self.generated_chunk_positions.push(hash);
        }
    }

    // Target structure tags persist source chunk and bounds/pieces, but generated chunk
    // bookkeeping is not serialized. Decode intentionally resets transient idempotence state.
    pub(crate) fn encode_core_semantics(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(32);
        for value in [
            self.source.x(),
            self.source.z(),
            self.bounds.x0,
            self.bounds.y0,
            self.bounds.z0,
            self.bounds.x1,
            self.bounds.y1,
            self.bounds.z1,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    pub(crate) fn decode_core_semantics(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 32 {
            return None;
        }
        let mut values = [0_i32; 8];
        for (index, value) in values.iter_mut().enumerate() {
            let start = index * 4;
            *value = i32::from_le_bytes(bytes[start..start + 4].try_into().ok()?);
        }
        Some(Self::new(
            ChunkCoord::new(values[0], values[1]),
            StructureBounds::new(
                values[2], values[3], values[4], values[5], values[6], values[7],
            ),
        ))
    }
}

pub(crate) fn structure_source_chunks(center: ChunkCoord, radius: i32) -> Vec<ChunkCoord> {
    let side = (radius * 2 + 1) as usize;
    let mut chunks = Vec::with_capacity(side * side);
    for x in center.x().wrapping_sub(radius)..=center.x().wrapping_add(radius) {
        for z in center.z().wrapping_sub(radius)..=center.z().wrapping_add(radius) {
            chunks.push(ChunkCoord::new(x, z));
        }
    }
    chunks
}

pub(crate) fn structure_source_random(seed: u32, source: ChunkCoord) -> MtRandom {
    let mut random = MtRandom::new(seed);
    let x_scale = odd_scale(random.next_positive_int());
    let z_scale = odd_scale(random.next_positive_int());
    let mixed = source
        .x()
        .wrapping_mul(x_scale)
        .wrapping_add(source.z().wrapping_mul(z_scale));
    random.reseed(u32::from_ne_bytes(mixed.to_ne_bytes()) ^ seed);
    random
}

pub(crate) const fn chunk_hash(x: i32, z: i32) -> i32 {
    x.wrapping_mul(0x1f1f_1f1f_u32 as i32) ^ z
}

const fn odd_scale(value: u32) -> i32 {
    let value = value as i32;
    (value / 2).wrapping_mul(2).wrapping_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_target_structure_feature_ids_are_exact() {
        assert_eq!(StructureFeatureKind::EndCity as i8, 0);
        assert_eq!(StructureFeatureKind::Fortress as i8, 1);
        assert_eq!(StructureFeatureKind::Mineshaft as i8, 2);
        assert_eq!(StructureFeatureKind::Monument as i8, 3);
        assert_eq!(StructureFeatureKind::Stronghold as i8, 4);
        assert_eq!(StructureFeatureKind::Temple as i8, 5);
        assert_eq!(StructureFeatureKind::Village as i8, 6);
    }

    #[test]
    fn bounding_box_orientation_matches_fixed_target_direction_ids() {
        let expected = [
            (
                StructureOrientation::South,
                StructureBounds::new(11, 22, 33, 15, 27, 39),
            ),
            (
                StructureOrientation::West,
                StructureBounds::new(7, 22, 31, 13, 27, 35),
            ),
            (
                StructureOrientation::North,
                StructureBounds::new(11, 22, 27, 15, 27, 33),
            ),
            (
                StructureOrientation::East,
                StructureBounds::new(13, 22, 31, 19, 27, 35),
            ),
        ];
        for (orientation, bounds) in expected {
            assert_eq!(
                StructureBounds::orient_box(10, 20, 30, 1, 2, 3, 5, 6, 7, orientation),
                bounds,
            );
        }
    }

    #[test]
    fn source_scan_and_chunk_hash_match_target() {
        let chunks = structure_source_chunks(ChunkCoord::new(-1, 2), 4);
        assert_eq!(chunks.len(), 81);
        assert_eq!(chunks.first(), Some(&ChunkCoord::new(-5, -2)));
        assert_eq!(chunks.last(), Some(&ChunkCoord::new(3, 6)));
        assert_eq!(chunk_hash(0, 0), 0);
        assert_eq!(chunk_hash(1, 0), 0x1f1f_1f1f);
        assert_eq!(chunk_hash(-1, -1), 0x1f1f_1f1e);
    }

    #[test]
    fn structure_core_round_trip_drops_transient_generated_chunk_bookkeeping() {
        let target = ChunkCoord::new(-3, 4);
        let mut start = StructureStartCore::new(
            ChunkCoord::new(-2, 4),
            StructureBounds::new(-48, 40, 64, -17, 75, 95),
        );
        assert!(start.should_post_process(target));
        start.mark_post_processed(target);
        assert!(!start.should_post_process(target));

        let decoded = StructureStartCore::decode_core_semantics(&start.encode_core_semantics())
            .expect("valid structure core");
        assert_eq!(decoded.source(), start.source());
        assert_eq!(decoded.bounds(), start.bounds());
        assert!(decoded.should_post_process(target));
    }
}
