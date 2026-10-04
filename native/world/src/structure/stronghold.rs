use crate::ChunkCoord;
use crate::terrain_shape::noise::MtRandom;

use super::stronghold_plan::{
    StrongholdDoor, StrongholdPieceExtra, StrongholdPieceKind, StrongholdPiecePlan, StrongholdPlan,
};
use super::village::VillageFeature;
use super::{
    StructureBounds, StructureOrientation, StructureStartCache, StructureStartCore,
    structure_source_chunks, structure_source_random,
};

const TOTAL_VILLAGE_STRONGHOLDS: usize = 3;
const GRID_SIZE: i32 = 200;
const GRID_INSET: i32 = 150;
const MIN_STRONGHOLD_DISTANCE: i32 = 10;
const STRONGHOLD_CHANCE: f32 = 0.25;
const ADDITIONAL_X_SCALE: i64 = 784_295_783_249;
const ADDITIONAL_Z_SCALE: i64 = 827_828_252_345;
const ADDITIONAL_SALT: i64 = 97_858_791;

pub(crate) struct StrongholdFeature {
    seed: u32,
    villages: VillageFeature,
    selected_chunks: Option<[ChunkCoord; TOTAL_VILLAGE_STRONGHOLDS]>,
}

impl StrongholdFeature {
    pub(crate) fn new(seed: i32) -> Self {
        Self {
            seed: u32::from_ne_bytes(seed.to_ne_bytes()),
            villages: VillageFeature::new(seed),
            selected_chunks: None,
        }
    }

    pub(crate) fn is_feature_chunk(&mut self, random: &mut MtRandom, source: ChunkCoord) -> bool {
        if self.selected_chunks.is_none() {
            self.generate_positions(random);
        }

        if self
            .selected_chunks
            .as_ref()
            .is_some_and(|positions| positions.contains(&source))
        {
            return true;
        }

        self.has_additional_stronghold(source)
    }

    #[cfg(test)]
    pub(crate) fn selected_chunks(&mut self) -> [ChunkCoord; TOTAL_VILLAGE_STRONGHOLDS] {
        if self.selected_chunks.is_none() {
            let mut random = MtRandom::new(self.seed);
            self.generate_positions(&mut random);
        }
        self.selected_chunks.expect("positions generated")
    }

    fn generate_positions(&mut self, random: &mut MtRandom) {
        random.reseed(self.seed);

        let mut positions = [ChunkCoord::new(0, 0); TOTAL_VILLAGE_STRONGHOLDS];
        let mut count = 0_usize;
        let mut angle = std::f32::consts::PI * 2.0 * random.next_float();
        let mut chunk_distance = 40_i32 + random.next_int(16) as i32;

        while count < TOTAL_VILLAGE_STRONGHOLDS {
            let center_x = (angle.cos() * chunk_distance as f32).floor() as i32;
            let center_z = (angle.sin() * chunk_distance as f32).floor() as i32;
            let mut found = None;

            'scan: for chunk_x in center_x - 8..center_x + 8 {
                for chunk_z in center_z - 8..center_z + 8 {
                    let source = ChunkCoord::new(chunk_x, chunk_z);
                    if self.villages.is_feature_chunk_with_random(source, random) {
                        found = Some(source);
                        break 'scan;
                    }
                }
            }

            if let Some(source) = found {
                positions[count] = source;
                count += 1;
                angle += std::f32::consts::PI * 0.6;
                chunk_distance += 8;
            } else {
                angle += std::f32::consts::PI * 0.25;
                chunk_distance += 4;
            }
        }

        self.selected_chunks = Some(positions);
    }

    fn has_additional_stronghold(&self, source: ChunkCoord) -> bool {
        if !beyond_minimum_distance(source) {
            return false;
        }

        let center = center_of_grid(source);
        let Some(location) = self.generate_additional_stronghold(center) else {
            return false;
        };
        location == source
    }

    fn generate_additional_stronghold(&self, center: ChunkCoord) -> Option<ChunkCoord> {
        if !beyond_minimum_distance(center) {
            return None;
        }

        let grid = grid_coordinates(center);
        let mixed = i64::from(center.x())
            .wrapping_mul(ADDITIONAL_X_SCALE)
            .wrapping_add(i64::from(center.z()).wrapping_mul(ADDITIONAL_Z_SCALE))
            .wrapping_add(i64::from(self.seed))
            .wrapping_add(ADDITIONAL_SALT);
        let mut random = MtRandom::new(mixed as u32);

        let min_x = grid
            .0
            .wrapping_mul(GRID_SIZE)
            .wrapping_add(GRID_SIZE - GRID_INSET);
        let max_x = grid.0.wrapping_mul(GRID_SIZE).wrapping_add(GRID_INSET);
        let min_z = grid
            .1
            .wrapping_mul(GRID_SIZE)
            .wrapping_add(GRID_SIZE - GRID_INSET);
        let max_z = grid.1.wrapping_mul(GRID_SIZE).wrapping_add(GRID_INSET);

        let x = min_x.wrapping_add(random.next_int((max_x - min_x) as u32) as i32);
        let z = min_z.wrapping_add(random.next_int((max_z - min_z) as u32) as i32);
        (random.next_float() < STRONGHOLD_CHANCE).then_some(ChunkCoord::new(x, z))
    }

    #[cfg(test)]
    fn additional_for_grid(&self, grid_x: i32, grid_z: i32) -> Option<ChunkCoord> {
        self.generate_additional_stronghold(ChunkCoord::new(
            grid_x.wrapping_mul(GRID_SIZE).wrapping_add(GRID_SIZE / 2),
            grid_z.wrapping_mul(GRID_SIZE).wrapping_add(GRID_SIZE / 2),
        ))
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct StrongholdStructureState {
    starts: StructureStartCache<StrongholdPlan>,
}

impl StrongholdStructureState {
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
        bytes.extend_from_slice(b"CSS1");
        bytes.extend_from_slice(&(self.starts.len() as u32).to_le_bytes());
        for start in self.starts.iter() {
            encode_plan(start, &mut bytes);
        }
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 8 || &bytes[..4] != b"CSS1" {
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

pub struct OverworldStrongholdStructures {
    seed: u32,
    feature: StrongholdFeature,
}

impl OverworldStrongholdStructures {
    pub fn new(seed: i32) -> Self {
        Self {
            seed: u32::from_ne_bytes(seed.to_ne_bytes()),
            feature: StrongholdFeature::new(seed),
        }
    }

    pub fn apply(&mut self, state: &mut StrongholdStructureState, target: ChunkCoord) {
        for source in structure_source_chunks(target, 8) {
            let mut random = structure_source_random(self.seed, source);
            let _ = random.next_positive_int();

            if !self.feature.is_feature_chunk(&mut random, source) {
                continue;
            }
            if state
                .starts
                .iter()
                .any(|start| start.core.source() == source)
            {
                continue;
            }

            state.starts.push(StrongholdPlan::generate(source, random));
        }
    }
}

fn encode_plan(plan: &StrongholdPlan, out: &mut Vec<u8>) {
    out.extend_from_slice(&plan.core.encode_core_semantics());
    out.extend_from_slice(&(plan.pieces.len() as u32).to_le_bytes());
    for piece in &plan.pieces {
        out.push(piece.kind as u8);
        out.push(piece.orientation as u8);
        out.push(piece.entry_door as u8);
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
        match piece.extra {
            StrongholdPieceExtra::None => out.push(0),
            StrongholdPieceExtra::StairsDown { is_source } => {
                out.push(1);
                out.push(u8::from(is_source));
            }
            StrongholdPieceExtra::ChestCorridor { has_placed_chest } => {
                out.push(2);
                out.push(u8::from(has_placed_chest));
            }
            StrongholdPieceExtra::FillerCorridor { steps } => {
                out.push(3);
                out.extend_from_slice(&steps.to_le_bytes());
            }
            StrongholdPieceExtra::FiveCrossing {
                left_high,
                left_low,
                right_high,
                right_low,
            } => {
                out.push(4);
                out.push(u8::from(left_high));
                out.push(u8::from(left_low));
                out.push(u8::from(right_high));
                out.push(u8::from(right_low));
            }
            StrongholdPieceExtra::Library { is_tall } => {
                out.push(5);
                out.push(u8::from(is_tall));
            }
            StrongholdPieceExtra::PortalRoom {
                has_placed_mob_spawner,
            } => {
                out.push(6);
                out.push(u8::from(has_placed_mob_spawner));
            }
            StrongholdPieceExtra::RoomCrossing { room_type } => {
                out.push(7);
                out.extend_from_slice(&room_type.to_le_bytes());
            }
            StrongholdPieceExtra::Straight {
                left_child,
                right_child,
            } => {
                out.push(8);
                out.push(u8::from(left_child));
                out.push(u8::from(right_child));
            }
        }
    }
}

fn decode_plan(cursor: &mut Cursor<'_>) -> Option<StrongholdPlan> {
    let core = StructureStartCore::decode_core_semantics(cursor.take(32)?)?;
    let count = cursor.u32()? as usize;
    if count == 0 || count > 4096 {
        return None;
    }

    let mut pieces = Vec::with_capacity(count);
    for _ in 0..count {
        let kind = match cursor.u8()? {
            0 => StrongholdPieceKind::Straight,
            1 => StrongholdPieceKind::PrisonHall,
            2 => StrongholdPieceKind::LeftTurn,
            3 => StrongholdPieceKind::RightTurn,
            4 => StrongholdPieceKind::RoomCrossing,
            5 => StrongholdPieceKind::StraightStairsDown,
            6 => StrongholdPieceKind::StairsDown,
            7 => StrongholdPieceKind::FiveCrossing,
            8 => StrongholdPieceKind::ChestCorridor,
            9 => StrongholdPieceKind::Library,
            10 => StrongholdPieceKind::PortalRoom,
            11 => StrongholdPieceKind::FillerCorridor,
            _ => return None,
        };
        let orientation = decode_orientation(cursor.u8()?)?;
        let entry_door = match cursor.u8()? {
            0 => StrongholdDoor::Opening,
            1 => StrongholdDoor::WoodDoor,
            2 => StrongholdDoor::Grates,
            3 => StrongholdDoor::IronDoor,
            _ => return None,
        };
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
            0 if matches!(
                kind,
                StrongholdPieceKind::PrisonHall
                    | StrongholdPieceKind::LeftTurn
                    | StrongholdPieceKind::RightTurn
                    | StrongholdPieceKind::StraightStairsDown
            ) =>
            {
                StrongholdPieceExtra::None
            }
            1 if kind == StrongholdPieceKind::StairsDown => StrongholdPieceExtra::StairsDown {
                is_source: cursor.u8()? != 0,
            },
            2 if kind == StrongholdPieceKind::ChestCorridor => {
                StrongholdPieceExtra::ChestCorridor {
                    has_placed_chest: cursor.u8()? != 0,
                }
            }
            3 if kind == StrongholdPieceKind::FillerCorridor => {
                StrongholdPieceExtra::FillerCorridor {
                    steps: cursor.i32()?,
                }
            }
            4 if kind == StrongholdPieceKind::FiveCrossing => StrongholdPieceExtra::FiveCrossing {
                left_high: cursor.u8()? != 0,
                left_low: cursor.u8()? != 0,
                right_high: cursor.u8()? != 0,
                right_low: cursor.u8()? != 0,
            },
            5 if kind == StrongholdPieceKind::Library => StrongholdPieceExtra::Library {
                is_tall: cursor.u8()? != 0,
            },
            6 if kind == StrongholdPieceKind::PortalRoom => StrongholdPieceExtra::PortalRoom {
                has_placed_mob_spawner: cursor.u8()? != 0,
            },
            7 if kind == StrongholdPieceKind::RoomCrossing => StrongholdPieceExtra::RoomCrossing {
                room_type: cursor.i32()?,
            },
            8 if kind == StrongholdPieceKind::Straight => StrongholdPieceExtra::Straight {
                left_child: cursor.u8()? != 0,
                right_child: cursor.u8()? != 0,
            },
            _ => return None,
        };
        pieces.push(StrongholdPiecePlan {
            kind,
            bounds,
            orientation,
            gen_depth,
            entry_door,
            extra,
        });
    }

    Some(StrongholdPlan { core, pieces })
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

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
}

fn beyond_minimum_distance(source: ChunkCoord) -> bool {
    let length_squared = source
        .x()
        .wrapping_mul(source.x())
        .wrapping_add(source.z().wrapping_mul(source.z()));
    length_squared >= MIN_STRONGHOLD_DISTANCE * MIN_STRONGHOLD_DISTANCE
}

fn grid_coordinates(source: ChunkCoord) -> (i32, i32) {
    (
        ((source.x() as f32) / GRID_SIZE as f32).floor() as i32,
        ((source.z() as f32) / GRID_SIZE as f32).floor() as i32,
    )
}

fn center_of_grid(source: ChunkCoord) -> ChunkCoord {
    let (grid_x, grid_z) = grid_coordinates(source);
    ChunkCoord::new(
        grid_x.wrapping_mul(GRID_SIZE).wrapping_add(GRID_SIZE / 2),
        grid_z.wrapping_mul(GRID_SIZE).wrapping_add(GRID_SIZE / 2),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structure::structure_source_random;

    #[test]
    fn independent_locator_fixtures_match_target_mt_and_raw_biomes() {
        // Independent oracle: std::mt19937 Stronghold/Village math plus cubiomes MC_1_8
        // pre-Voronoi biome viability through a C bridge.
        let fixtures = [
            (
                0,
                [
                    ChunkCoord::new(-55, -25),
                    ChunkCoord::new(12, 101),
                    ChunkCoord::new(-112, -155),
                ],
            ),
            (
                -1,
                [
                    ChunkCoord::new(-381, 530),
                    ChunkCoord::new(102, -663),
                    ChunkCoord::new(-320, 646),
                ],
            ),
            (
                i32::MIN,
                [
                    ChunkCoord::new(5, -68),
                    ChunkCoord::new(96, -38),
                    ChunkCoord::new(8, -160),
                ],
            ),
            (
                0x1234_5678,
                [
                    ChunkCoord::new(223, -160),
                    ChunkCoord::new(-117, -376),
                    ChunkCoord::new(426, -420),
                ],
            ),
        ];

        for (seed, expected) in fixtures {
            assert_eq!(StrongholdFeature::new(seed).selected_chunks(), expected);
        }

        let feature0 = StrongholdFeature::new(0);
        assert_eq!(
            feature0.additional_for_grid(1, 0),
            Some(ChunkCoord::new(321, 93))
        );
        assert_eq!(feature0.additional_for_grid(-1, -1), None);

        let feature_neg = StrongholdFeature::new(-1);
        assert_eq!(feature_neg.additional_for_grid(2, -3), None);

        let feature_min = StrongholdFeature::new(i32::MIN);
        assert_eq!(
            feature_min.additional_for_grid(-2, 1),
            Some(ChunkCoord::new(-345, 316))
        );
    }

    #[test]
    fn runtime_discovers_selected_start_and_state_round_trips_idempotently() {
        let source = ChunkCoord::new(-55, -25);
        let mut runtime = OverworldStrongholdStructures::new(0);
        let mut state = StrongholdStructureState::new();
        runtime.apply(&mut state, source);
        assert!(!state.is_empty());
        let count = state.len();

        runtime.apply(&mut state, source);
        assert_eq!(state.len(), count);

        let encoded = state.encode();
        let decoded = StrongholdStructureState::decode(&encoded).expect("valid Stronghold state");
        assert_eq!(decoded.encode(), encoded);
    }

    #[test]
    fn state_codec_rejects_truncation_and_trailing_bytes() {
        let source = ChunkCoord::new(-55, -25);
        let mut runtime = OverworldStrongholdStructures::new(0);
        let mut state = StrongholdStructureState::new();
        runtime.apply(&mut state, source);
        let encoded = state.encode();
        assert!(StrongholdStructureState::decode(&encoded[..encoded.len() - 1]).is_none());
        let mut trailing = encoded;
        trailing.push(0);
        assert!(StrongholdStructureState::decode(&trailing).is_none());
    }

    #[test]
    fn additional_grid_locations_stay_inside_target_inset() {
        let feature = StrongholdFeature::new(0);
        for grid_z in -4..=4 {
            for grid_x in -4..=4 {
                let Some(source) = feature.additional_for_grid(grid_x, grid_z) else {
                    continue;
                };
                let base_x = grid_x * GRID_SIZE;
                let base_z = grid_z * GRID_SIZE;
                assert!((base_x + 50..base_x + 150).contains(&source.x()));
                assert!((base_z + 50..base_z + 150).contains(&source.z()));
            }
        }
    }

    #[test]
    fn first_position_generation_mutates_the_supplied_structure_stream_once() {
        let mut feature = StrongholdFeature::new(0);
        let probe = ChunkCoord::new(-8, -8);
        let mut first = structure_source_random(0, probe);
        let _ = first.next_positive_int();
        let before = first.clone();
        let _ = feature.is_feature_chunk(&mut first, probe);

        let mut expected_tail = MtRandom::new(0);
        feature.generate_positions(&mut expected_tail);
        assert_eq!(first.next_u32(), expected_tail.next_u32());

        let mut second = before;
        let _ = feature.is_feature_chunk(&mut second, probe);
        assert_ne!(second.next_u32(), expected_tail.next_u32());
    }

    #[test]
    fn selected_sources_are_village_sources_and_additional_sources_are_grid_local() {
        let mut feature = StrongholdFeature::new(-1);
        let selected = feature.selected_chunks();
        let villages = VillageFeature::new(-1);
        assert!(
            selected
                .into_iter()
                .all(|source| villages.is_feature_chunk(source))
        );

        let mut found_additional = false;
        for gz in -5..=5 {
            for gx in -5..=5 {
                if feature.additional_for_grid(gx, gz).is_some() {
                    found_additional = true;
                }
            }
        }
        assert!(found_additional);
    }
}
