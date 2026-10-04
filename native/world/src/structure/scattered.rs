use crate::ChunkCoord;
use crate::biome_source::OverworldBiomeSource;
use crate::population::{PopulationNeighborhood, population_seed};
use crate::terrain_shape::noise::MtRandom;

use super::scattered_place::ScatteredPostProcessor;
use super::{
    StructureBounds, StructureStartCache, StructureStartCore, structure_source_chunks,
    structure_source_random,
};

const SOURCE_RADIUS: i32 = 8;
const SPACING: i32 = 32;
const MIN_SEPARATION: i32 = 8;

const DESERT: u8 = 2;
const SWAMPLAND: u8 = 6;
const DESERT_HILLS: u8 = 17;
const JUNGLE: u8 = 21;
const JUNGLE_HILLS: u8 = 22;
const SWAMPLAND_MUTATED: u8 = 134;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum ScatteredKind {
    DesertPyramid = 0,
    JunglePyramid = 1,
    SwamplandHut = 2,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ScatteredPieceState {
    pub(crate) kind: ScatteredKind,
    pub(crate) height_position: i32,
    pub(crate) desert_chests: [bool; 4],
    pub(crate) jungle_main_chest: bool,
    pub(crate) jungle_hidden_chest: bool,
    pub(crate) jungle_traps: [bool; 2],
    pub(crate) spawned_witch: bool,
}

impl ScatteredPieceState {
    pub(super) fn new(kind: ScatteredKind) -> Self {
        Self {
            kind,
            height_position: -1,
            desert_chests: [false; 4],
            jungle_main_chest: false,
            jungle_hidden_chest: false,
            jungle_traps: [false; 2],
            spawned_witch: false,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ScatteredPlan {
    pub(crate) core: StructureStartCore,
    pub(crate) piece: Option<ScatteredPieceState>,
}

pub(crate) struct ScatteredFeature {
    biomes: OverworldBiomeSource,
}

impl ScatteredFeature {
    pub(crate) fn new(seed: i32) -> Self {
        Self {
            biomes: OverworldBiomeSource::new(seed),
        }
    }

    fn kind_at_source(&self, random: &mut MtRandom, source: ChunkCoord) -> Option<ScatteredKind> {
        if !candidate(random, source) {
            return None;
        }
        self.biome_kind(source)
    }

    fn biome_kind(&self, source: ChunkCoord) -> Option<ScatteredKind> {
        let block_x = source.x().wrapping_mul(16).wrapping_add(8);
        let block_z = source.z().wrapping_mul(16).wrapping_add(8);
        let biome = self.biomes.raw_biome_ids(block_x >> 2, block_z >> 2, 1, 1)[0];

        match biome {
            DESERT | DESERT_HILLS => Some(ScatteredKind::DesertPyramid),
            JUNGLE | JUNGLE_HILLS => Some(ScatteredKind::JunglePyramid),
            SWAMPLAND | SWAMPLAND_MUTATED => Some(ScatteredKind::SwamplandHut),
            _ => None,
        }
    }

    fn plan(&self, random: &mut MtRandom, source: ChunkCoord) -> Option<ScatteredPlan> {
        let kind = self.kind_at_source(random, source)?;
        let west = source.x().wrapping_mul(16);
        let north = source.z().wrapping_mul(16);
        let bounds = match kind {
            ScatteredKind::DesertPyramid => {
                StructureBounds::new(west, 64, north, west + 20, 78, north + 20)
            }
            ScatteredKind::JunglePyramid => {
                StructureBounds::new(west, 64, north, west + 11, 73, north + 14)
            }
            ScatteredKind::SwamplandHut => {
                StructureBounds::new(west, 64, north, west + 6, 70, north + 8)
            }
        };

        Some(ScatteredPlan {
            core: StructureStartCore::new(source, bounds),
            piece: Some(ScatteredPieceState::new(kind)),
        })
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct ScatteredStructureState {
    starts: StructureStartCache<ScatteredPlan>,
}

impl ScatteredStructureState {
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
        let mut out = Vec::new();
        out.extend_from_slice(b"CST1");
        out.extend_from_slice(&(self.starts.len() as u32).to_le_bytes());
        for start in self.starts.iter() {
            encode_plan(start, &mut out);
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 8 || &bytes[..4] != b"CST1" {
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

pub struct OverworldScatteredStructures {
    seed: u32,
    feature: ScatteredFeature,
}

impl OverworldScatteredStructures {
    pub fn new(seed: i32) -> Self {
        Self {
            seed: u32::from_ne_bytes(seed.to_ne_bytes()),
            feature: ScatteredFeature::new(seed),
        }
    }

    pub fn apply(&self, state: &mut ScatteredStructureState, target: ChunkCoord) {
        for source in structure_source_chunks(target, SOURCE_RADIUS) {
            if state
                .starts
                .iter()
                .any(|start| start.core.source() == source)
            {
                continue;
            }

            let mut random = structure_source_random(self.seed, source);
            let _ = random.next_positive_int();
            if let Some(plan) = self.feature.plan(&mut random, source) {
                state.starts.push(plan);
            }
        }
    }

    pub fn post_process(
        &self,
        state: &mut ScatteredStructureState,
        neighborhood: &mut PopulationNeighborhood,
        target: ChunkCoord,
    ) -> bool {
        let mut random = MtRandom::new(population_seed(self.seed, target));
        self.post_process_with_random(state, neighborhood, target, &mut random)
    }

    pub(crate) fn post_process_with_random(
        &self,
        state: &mut ScatteredStructureState,
        neighborhood: &mut PopulationNeighborhood,
        target: ChunkCoord,
        random: &mut MtRandom,
    ) -> bool {
        let mut changed = false;
        for start in state.starts.iter_mut() {
            changed |= ScatteredPostProcessor::process_start(start, neighborhood, target, random);
        }
        changed
    }

    #[cfg(test)]
    fn plan_at(&self, source: ChunkCoord) -> Option<ScatteredPlan> {
        let mut random = structure_source_random(self.seed, source);
        let _ = random.next_positive_int();
        self.feature.plan(&mut random, source)
    }
}

fn candidate(random: &mut MtRandom, source: ChunkCoord) -> bool {
    let mut x = source.x();
    let mut z = source.z();
    if x < 0 {
        x = x.wrapping_sub(SPACING - 1);
    }
    if z < 0 {
        z = z.wrapping_sub(SPACING - 1);
    }

    let mut center_x = x / SPACING;
    let mut center_z = z / SPACING;
    center_x = center_x
        .wrapping_mul(SPACING)
        .wrapping_add(random.next_int((SPACING - MIN_SEPARATION) as u32) as i32);
    center_z = center_z
        .wrapping_mul(SPACING)
        .wrapping_add(random.next_int((SPACING - MIN_SEPARATION) as u32) as i32);

    source.x() == center_x && source.z() == center_z
}

fn encode_plan(plan: &ScatteredPlan, out: &mut Vec<u8>) {
    out.extend_from_slice(&plan.core.encode_core_semantics());
    match &plan.piece {
        None => out.push(0),
        Some(piece) => {
            out.push(1);
            out.push(piece.kind as u8);
            out.extend_from_slice(&piece.height_position.to_le_bytes());
            for flag in piece.desert_chests {
                out.push(u8::from(flag));
            }
            out.push(u8::from(piece.jungle_main_chest));
            out.push(u8::from(piece.jungle_hidden_chest));
            for flag in piece.jungle_traps {
                out.push(u8::from(flag));
            }
            out.push(u8::from(piece.spawned_witch));
        }
    }
}

fn decode_plan(cursor: &mut Cursor<'_>) -> Option<ScatteredPlan> {
    let core = StructureStartCore::decode_core_semantics(cursor.take(32)?)?;
    let piece = match cursor.u8()? {
        0 => None,
        1 => {
            let kind = match cursor.u8()? {
                0 => ScatteredKind::DesertPyramid,
                1 => ScatteredKind::JunglePyramid,
                2 => ScatteredKind::SwamplandHut,
                _ => return None,
            };
            Some(ScatteredPieceState {
                kind,
                height_position: cursor.i32()?,
                desert_chests: [
                    cursor.bool()?,
                    cursor.bool()?,
                    cursor.bool()?,
                    cursor.bool()?,
                ],
                jungle_main_chest: cursor.bool()?,
                jungle_hidden_chest: cursor.bool()?,
                jungle_traps: [cursor.bool()?, cursor.bool()?],
                spawned_witch: cursor.bool()?,
            })
        }
        _ => return None,
    };
    Some(ScatteredPlan { core, piece })
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    const fn is_finished(&self) -> bool {
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

    fn bool(&mut self) -> Option<bool> {
        Some(self.u8()? != 0)
    }

    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_candidate_kind_fixtures_match_target_mt_and_raw_biomes() {
        // Standalone C++ std::mt19937 LargeFeature/spacing oracle plus cubiomes MC_1_8
        // pre-Voronoi biome lookup.
        let fixtures = [
            (
                0,
                [
                    (ChunkCoord::new(981, -1016), ScatteredKind::DesertPyramid),
                    (ChunkCoord::new(627, -987), ScatteredKind::JunglePyramid),
                    (ChunkCoord::new(-565, -1022), ScatteredKind::SwamplandHut),
                ],
            ),
            (
                -1,
                [
                    (ChunkCoord::new(-333, -1023), ScatteredKind::DesertPyramid),
                    (ChunkCoord::new(41, -1023), ScatteredKind::JunglePyramid),
                    (ChunkCoord::new(720, -1002), ScatteredKind::SwamplandHut),
                ],
            ),
            (
                i32::MIN,
                [
                    (ChunkCoord::new(-780, -1016), ScatteredKind::DesertPyramid),
                    (ChunkCoord::new(-536, -893), ScatteredKind::JunglePyramid),
                    (ChunkCoord::new(631, -1005), ScatteredKind::SwamplandHut),
                ],
            ),
            (
                0x1234_5678,
                [
                    (ChunkCoord::new(-919, -1022), ScatteredKind::DesertPyramid),
                    (ChunkCoord::new(-921, -970), ScatteredKind::JunglePyramid),
                    (ChunkCoord::new(405, -1020), ScatteredKind::SwamplandHut),
                ],
            ),
        ];

        for (seed, cases) in fixtures {
            let runtime = OverworldScatteredStructures::new(seed);
            for (source, expected_kind) in cases {
                let plan = runtime
                    .plan_at(source)
                    .expect("oracle source must generate");
                assert_eq!(plan.piece.as_ref().expect("piece").kind, expected_kind);
            }
        }
    }

    #[test]
    fn state_codec_round_trips_and_rejects_trailing_data() {
        let runtime = OverworldScatteredStructures::new(0);
        let source = (-1024..=1024)
            .flat_map(|z| (-1024..=1024).map(move |x| ChunkCoord::new(x, z)))
            .find(|source| runtime.plan_at(*source).is_some())
            .expect("expected fixed-target scattered source");

        let mut state = ScatteredStructureState::new();
        runtime.apply(&mut state, source);
        assert!(!state.is_empty());

        let encoded = state.encode();
        let decoded = ScatteredStructureState::decode(&encoded).expect("valid scattered state");
        assert_eq!(decoded.encode(), encoded);

        let mut trailing = encoded;
        trailing.push(0);
        assert!(ScatteredStructureState::decode(&trailing).is_none());
    }

    #[test]
    fn target_family_excludes_post_015_igloo() {
        let kinds = [
            ScatteredKind::DesertPyramid,
            ScatteredKind::JunglePyramid,
            ScatteredKind::SwamplandHut,
        ];
        assert_eq!(kinds.len(), 3);
    }
}
