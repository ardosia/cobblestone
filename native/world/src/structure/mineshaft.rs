use crate::ChunkCoord;
use crate::biome_source::OverworldBiomeSource;
use crate::population::{PopulationNeighborhood, population_seed};
use crate::terrain_shape::noise::MtRandom;

use super::mineshaft_place::MineshaftPostProcessor;
use super::mineshaft_plan::{
    MineshaftPieceExtra, MineshaftPieceKind, MineshaftPiecePlan, MineshaftPlan,
};
use super::{
    StructureBounds, StructureOrientation, StructureStartCache, StructureStartCore,
    structure_source_chunks, structure_source_random,
};

const MINESHAFT_SOURCE_RADIUS: i32 = 8;
const MESA_IDS: [u8; 6] = [37, 38, 39, 165, 166, 167];

pub(crate) struct MineshaftFeature {
    seed: u32,
    biomes: OverworldBiomeSource,
}

impl MineshaftFeature {
    pub(crate) fn new(seed: i32) -> Self {
        Self {
            seed: u32::from_ne_bytes(seed.to_ne_bytes()),
            biomes: OverworldBiomeSource::new(seed),
        }
    }

    pub(crate) fn plan(&self, source: ChunkCoord) -> Option<MineshaftPlan> {
        let mut random = structure_source_random(self.seed, source);
        let _ = random.next_positive_int();
        if !candidate(&mut random, source) {
            return None;
        }

        let block_x = source.x().wrapping_mul(16);
        let block_z = source.z().wrapping_mul(16);
        let biome = self.biomes.biome_ids(block_x, block_z, 1, 1)[0];
        let surface = MESA_IDS.contains(&biome);
        Some(MineshaftPlan::generate(source, random, surface))
    }

    #[cfg(test)]
    pub(crate) fn is_feature_chunk(&self, source: ChunkCoord) -> bool {
        self.plan(source).is_some()
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct MineshaftStructureState {
    starts: StructureStartCache<MineshaftPlan>,
}

impl MineshaftStructureState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.starts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.starts.is_empty()
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"CMS1");
        bytes.extend_from_slice(&(self.starts.len() as u32).to_le_bytes());
        for start in self.starts.iter() {
            encode_plan(start, &mut bytes);
        }
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 8 || &bytes[..4] != b"CMS1" {
            return None;
        }
        let count = u32::from_le_bytes(bytes[4..8].try_into().ok()?) as usize;
        if count > 4096 {
            return None;
        }
        let mut cursor = Cursor::new(&bytes[8..]);
        let mut starts = Vec::with_capacity(count);
        for _ in 0..count {
            starts.push(decode_plan(&mut cursor)?);
        }
        cursor.is_finished().then_some(Self {
            starts: StructureStartCache::from_starts(starts),
        })
    }
}

pub struct OverworldMineshaftStructures {
    seed: u32,
    feature: MineshaftFeature,
}

impl OverworldMineshaftStructures {
    pub fn new(seed: i32) -> Self {
        Self {
            seed: u32::from_ne_bytes(seed.to_ne_bytes()),
            feature: MineshaftFeature::new(seed),
        }
    }

    pub fn apply(&self, state: &mut MineshaftStructureState, target: ChunkCoord) {
        for source in structure_source_chunks(target, MINESHAFT_SOURCE_RADIUS) {
            if state
                .starts
                .iter()
                .any(|start| start.core.source() == source)
            {
                continue;
            }
            if let Some(plan) = self.feature.plan(source) {
                state.starts.push(plan);
            }
        }
    }

    pub fn post_process(
        &self,
        state: &mut MineshaftStructureState,
        neighborhood: &mut PopulationNeighborhood,
        target: ChunkCoord,
    ) -> bool {
        let mut random = MtRandom::new(population_seed(self.seed, target));
        self.post_process_with_random(state, neighborhood, target, &mut random)
    }

    pub(crate) fn post_process_with_random(
        &self,
        state: &mut MineshaftStructureState,
        neighborhood: &mut PopulationNeighborhood,
        target: ChunkCoord,
        random: &mut MtRandom,
    ) -> bool {
        let mut changed = false;
        for start in state.starts.iter_mut() {
            changed |= MineshaftPostProcessor::process_start(start, neighborhood, target, random);
        }
        changed
    }

    #[cfg(test)]
    fn discover_sources_for_target(&self, target: ChunkCoord) -> Vec<ChunkCoord> {
        structure_source_chunks(target, MINESHAFT_SOURCE_RADIUS)
            .into_iter()
            .filter(|source| self.feature.is_feature_chunk(*source))
            .collect()
    }
}

fn candidate(random: &mut MtRandom, source: ChunkCoord) -> bool {
    if random.next_float() >= 0.004 {
        return false;
    }
    let distance = source.x().wrapping_abs().max(source.z().wrapping_abs());
    random.next_int(80) < distance as u32
}

pub(super) fn encode_plan(plan: &MineshaftPlan, out: &mut Vec<u8>) {
    out.extend_from_slice(&plan.core.encode_core_semantics());
    out.push(u8::from(plan.surface));
    out.extend_from_slice(&(plan.pieces.len() as u32).to_le_bytes());
    for piece in &plan.pieces {
        out.push(piece.kind as u8);
        out.push(piece.orientation as u8);
        out.extend_from_slice(&piece.gen_depth.to_le_bytes());
        for value in [
            piece.bounds.x0,
            piece.bounds.y0,
            piece.bounds.z0,
            piece.bounds.x1,
            piece.bounds.y1,
            piece.bounds.z1,
        ] {
            out.extend_from_slice(&value.to_le_bytes());
        }
        match &piece.extra {
            MineshaftPieceExtra::Room { entrances } => {
                out.push(0);
                out.extend_from_slice(&(entrances.len() as u32).to_le_bytes());
                for entrance in entrances {
                    for value in [
                        entrance.x0,
                        entrance.y0,
                        entrance.z0,
                        entrance.x1,
                        entrance.y1,
                        entrance.z1,
                    ] {
                        out.extend_from_slice(&value.to_le_bytes());
                    }
                }
            }
            MineshaftPieceExtra::Corridor {
                has_rails,
                spider_corridor,
                has_placed_spider,
                num_sections,
            } => {
                out.push(1);
                out.push(u8::from(*has_rails));
                out.push(u8::from(*spider_corridor));
                out.push(u8::from(*has_placed_spider));
                out.extend_from_slice(&num_sections.to_le_bytes());
            }
            MineshaftPieceExtra::Crossing {
                direction,
                two_floored,
            } => {
                out.push(2);
                out.push(*direction as u8);
                out.push(u8::from(*two_floored));
            }
            MineshaftPieceExtra::Stairs => out.push(3),
        }
    }
}

pub(super) fn decode_plan(cursor: &mut Cursor<'_>) -> Option<MineshaftPlan> {
    let core = StructureStartCore::decode_core_semantics(cursor.take(32)?)?;
    let surface = cursor.u8()? != 0;
    let count = cursor.u32()? as usize;
    if count > 4096 {
        return None;
    }
    let mut pieces = Vec::with_capacity(count);
    for _ in 0..count {
        let kind = match cursor.u8()? {
            0 => MineshaftPieceKind::Room,
            1 => MineshaftPieceKind::Corridor,
            2 => MineshaftPieceKind::Crossing,
            3 => MineshaftPieceKind::Stairs,
            _ => return None,
        };
        let orientation = decode_orientation(cursor.u8()?)?;
        let gen_depth = cursor.i32()?;
        let bounds = StructureBounds::new(
            cursor.i32()?,
            cursor.i32()?,
            cursor.i32()?,
            cursor.i32()?,
            cursor.i32()?,
            cursor.i32()?,
        );
        let extra = match cursor.u8()? {
            0 if kind == MineshaftPieceKind::Room => {
                let entrance_count = cursor.u32()? as usize;
                if entrance_count > 4096 {
                    return None;
                }
                let mut entrances = Vec::with_capacity(entrance_count);
                for _ in 0..entrance_count {
                    entrances.push(StructureBounds::new(
                        cursor.i32()?,
                        cursor.i32()?,
                        cursor.i32()?,
                        cursor.i32()?,
                        cursor.i32()?,
                        cursor.i32()?,
                    ));
                }
                MineshaftPieceExtra::Room { entrances }
            }
            1 if kind == MineshaftPieceKind::Corridor => MineshaftPieceExtra::Corridor {
                has_rails: cursor.u8()? != 0,
                spider_corridor: cursor.u8()? != 0,
                has_placed_spider: cursor.u8()? != 0,
                num_sections: cursor.i32()?,
            },
            2 if kind == MineshaftPieceKind::Crossing => MineshaftPieceExtra::Crossing {
                direction: decode_orientation(cursor.u8()?)?,
                two_floored: cursor.u8()? != 0,
            },
            3 if kind == MineshaftPieceKind::Stairs => MineshaftPieceExtra::Stairs,
            _ => return None,
        };
        pieces.push(MineshaftPiecePlan {
            kind,
            bounds,
            orientation,
            gen_depth,
            extra,
        });
    }
    Some(MineshaftPlan {
        core,
        surface,
        pieces,
    })
}

fn decode_orientation(value: u8) -> Option<StructureOrientation> {
    Some(match value {
        0 => StructureOrientation::South,
        1 => StructureOrientation::West,
        2 => StructureOrientation::North,
        3 => StructureOrientation::East,
        _ => return None,
    })
}

pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    pub(super) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub(super) const fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }

    fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        let end = self.offset.checked_add(count)?;
        let value = self.bytes.get(self.offset..end)?;
        self.offset = end;
        Some(value)
    }

    fn u8(&mut self) -> Option<u8> {
        Some(*self.take(1)?.first()?)
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_source(feature: &MineshaftFeature) -> ChunkCoord {
        (-512..=512)
            .flat_map(|z| (-512..=512).map(move |x| ChunkCoord::new(x, z)))
            .find(|source| feature.is_feature_chunk(*source))
            .expect("expected fixed-target Mineshaft source")
    }

    fn independent_plan(seed: i32, source: ChunkCoord, surface: bool) -> MineshaftPlan {
        let seed_bits = u32::from_ne_bytes(seed.to_ne_bytes());
        let mut random = structure_source_random(seed_bits, source);
        let _ = random.next_positive_int();
        assert!(candidate(&mut random, source));
        MineshaftPlan::generate(source, random, surface)
    }

    #[test]
    fn independent_candidate_and_topology_fixtures_match() {
        // Standalone C++ std::mt19937 oracle reproducing LargeFeature + StructureFeature
        // source reseeding and the fixed-target Mineshaft piece graph.
        let fixtures = [
            (
                0,
                ChunkCoord::new(-453, -512),
                false,
                0x7444_47f7_768d_9711_u64,
                129,
            ),
            (
                -1,
                ChunkCoord::new(-122, -512),
                false,
                0x1645_beda_8ffa_fd61_u64,
                100,
            ),
            (
                i32::MIN,
                ChunkCoord::new(-58, -512),
                false,
                0xa040_f0ce_79cf_ed0c_u64,
                149,
            ),
            (
                0,
                ChunkCoord::new(-453, -512),
                true,
                0x67d8_8cde_cae7_9670_u64,
                1,
            ),
        ];

        for (seed, source, surface, expected_hash, expected_count) in fixtures {
            let plan = independent_plan(seed, source, surface);
            assert_eq!(
                plan.hash_topology(),
                expected_hash,
                "seed={seed} source={source:?} surface={surface}"
            );
            assert_eq!(plan.pieces.len(), expected_count);
        }

        for (seed, candidates) in [
            (0, [ChunkCoord::new(-453, -512), ChunkCoord::new(-36, -512)]),
            (-1, [ChunkCoord::new(-122, -512), ChunkCoord::new(83, -512)]),
            (
                i32::MIN,
                [ChunkCoord::new(-58, -512), ChunkCoord::new(76, -512)],
            ),
        ] {
            let seed_bits = u32::from_ne_bytes(seed.to_ne_bytes());
            for source in candidates {
                let mut random = structure_source_random(seed_bits, source);
                let _ = random.next_positive_int();
                assert!(
                    candidate(&mut random, source),
                    "seed={seed} source={source:?}"
                );
            }
        }
    }

    #[test]
    fn candidate_discovery_uses_target_radius_eight() {
        let runtime = OverworldMineshaftStructures::new(0);
        let center = first_source(&runtime.feature);
        let sources = runtime.discover_sources_for_target(center);
        assert!(sources.contains(&center));
        assert!(sources.iter().all(|source| {
            source.x().wrapping_sub(center.x()).wrapping_abs() <= MINESHAFT_SOURCE_RADIUS
                && source.z().wrapping_sub(center.z()).wrapping_abs() <= MINESHAFT_SOURCE_RADIUS
        }));
    }

    #[test]
    fn durable_state_round_trips_and_repeated_apply_does_not_duplicate_starts() {
        let runtime = OverworldMineshaftStructures::new(0);
        let source = first_source(&runtime.feature);
        let mut state = MineshaftStructureState::new();
        runtime.apply(&mut state, source);
        assert!(!state.is_empty());
        let count = state.len();
        runtime.apply(&mut state, source);
        assert_eq!(state.len(), count);

        let encoded = state.encode();
        let decoded = MineshaftStructureState::decode(&encoded).expect("valid Mineshaft state");
        assert_eq!(decoded.encode(), encoded);
    }

    #[test]
    fn state_codec_rejects_truncated_and_trailing_bytes() {
        let runtime = OverworldMineshaftStructures::new(0);
        let source = first_source(&runtime.feature);
        let mut state = MineshaftStructureState::new();
        runtime.apply(&mut state, source);
        let encoded = state.encode();
        assert!(MineshaftStructureState::decode(&encoded[..encoded.len() - 1]).is_none());
        let mut trailing = encoded;
        trailing.push(0);
        assert!(MineshaftStructureState::decode(&trailing).is_none());
    }
}
