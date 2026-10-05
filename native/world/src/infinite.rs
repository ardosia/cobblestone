use std::array;
use std::collections::BTreeMap;

use crate::population::{
    PopulationChunkPlanes, PopulationNeighborhood, generation_height_map, population_random,
};
use crate::population_feature::is_leaves;
use crate::population_finalizer::material_blocks_motion;
use crate::{
    CHUNK_LIFECYCLE_GENERATED, CHUNK_LIFECYCLE_LIGHT_POPULATED, CHUNK_LIFECYCLE_POPULATED,
    CHUNK_NIBBLE_BYTES, ChunkCoord, ChunkImport, ChunkSnapshot, MineshaftStructureState,
    OverworldBiomeDecorator, OverworldBiomeSource, OverworldCaveCarver,
    OverworldFreezeFrostPopulator, OverworldLakePopulator, OverworldMineshaftStructures,
    OverworldMonsterRoomPopulator, OverworldPostDecorationFinalizer, OverworldScatteredStructures,
    OverworldStrongholdStructures, OverworldVillageStructures, ScatteredStructureState,
    StrongholdStructureState, VillageStructureState, WorldStore, WorldStoreError, biome_id,
    default_biome_word, linear_index_to_extra_key,
};

const STATE_MAGIC: &[u8; 4] = b"CIG1";
const FULL_LIFECYCLE: u8 =
    CHUNK_LIFECYCLE_GENERATED | CHUNK_LIFECYCLE_POPULATED | CHUNK_LIFECYCLE_LIGHT_POPULATED;

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct OverworldInfiniteState {
    village: VillageStructureState,
    mineshaft: MineshaftStructureState,
    stronghold: StrongholdStructureState,
    scattered: ScatteredStructureState,
}

impl OverworldInfiniteState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn encode(&self) -> Vec<u8> {
        let sections = [
            self.village.encode(),
            self.mineshaft.encode(),
            self.stronghold.encode(),
            self.scattered.encode(),
        ];
        let total = sections
            .iter()
            .fold(4_usize, |size, section| size + 4 + section.len());
        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(STATE_MAGIC);
        for section in sections {
            out.extend_from_slice(&(section.len() as u32).to_le_bytes());
            out.extend_from_slice(&section);
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 4 || &bytes[..4] != STATE_MAGIC {
            return None;
        }
        let mut offset = 4;
        let village = take_section(bytes, &mut offset).and_then(VillageStructureState::decode)?;
        let mineshaft =
            take_section(bytes, &mut offset).and_then(MineshaftStructureState::decode)?;
        let stronghold =
            take_section(bytes, &mut offset).and_then(StrongholdStructureState::decode)?;
        let scattered =
            take_section(bytes, &mut offset).and_then(ScatteredStructureState::decode)?;
        (offset == bytes.len()).then_some(Self {
            village,
            mineshaft,
            stronghold,
            scattered,
        })
    }
}

fn take_section<'a>(bytes: &'a [u8], offset: &mut usize) -> Option<&'a [u8]> {
    let length_end = offset.checked_add(4)?;
    let length = u32::from_le_bytes(bytes.get(*offset..length_end)?.try_into().ok()?) as usize;
    let payload_end = length_end.checked_add(length)?;
    let payload = bytes.get(length_end..payload_end)?;
    *offset = payload_end;
    Some(payload)
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct InfiniteGenerationResult {
    changed_chunks: usize,
    generated_center: bool,
}

impl InfiniteGenerationResult {
    pub const fn changed_chunks(self) -> usize {
        self.changed_chunks
    }

    pub const fn generated_center(self) -> bool {
        self.generated_center
    }
}

pub struct OverworldInfiniteGenerator {
    seed: i32,
    seed_bits: u32,
    carver: OverworldCaveCarver,
    lakes: OverworldLakePopulator,
    villages: OverworldVillageStructures,
    mineshafts: OverworldMineshaftStructures,
    strongholds: OverworldStrongholdStructures,
    scattered: OverworldScatteredStructures,
    monster_rooms: OverworldMonsterRoomPopulator,
    freeze: OverworldFreezeFrostPopulator,
    biome_decorator: OverworldBiomeDecorator,
    finalizer: OverworldPostDecorationFinalizer,
    state: OverworldInfiniteState,
}

impl OverworldInfiniteGenerator {
    pub fn new(seed: i32) -> Self {
        Self::with_state(seed, OverworldInfiniteState::new())
    }

    pub fn with_state(seed: i32, state: OverworldInfiniteState) -> Self {
        Self {
            seed,
            seed_bits: u32::from_ne_bytes(seed.to_ne_bytes()),
            carver: OverworldCaveCarver::new(seed),
            lakes: OverworldLakePopulator::new(seed),
            villages: OverworldVillageStructures::new(seed),
            mineshafts: OverworldMineshaftStructures::new(seed),
            strongholds: OverworldStrongholdStructures::new(seed),
            scattered: OverworldScatteredStructures::new(seed),
            monster_rooms: OverworldMonsterRoomPopulator::new(seed),
            freeze: OverworldFreezeFrostPopulator::new(),
            biome_decorator: OverworldBiomeDecorator::new(seed),
            finalizer: OverworldPostDecorationFinalizer::new(),
            state,
        }
    }

    pub const fn seed(&self) -> i32 {
        self.seed
    }

    pub fn state(&self) -> &OverworldInfiniteState {
        &self.state
    }

    /// Generates/populates one center chunk using the resident 3x3 neighborhood as authoritative
    /// input. Missing or not-yet-generated neighbors are initialized through the exact cave stage.
    ///
    /// Persistent callers must preflight all nine positions against storage before invoking this
    /// method so an unloaded on-disk neighbor is never mistaken for a missing chunk.
    pub fn generate_into_store(
        &mut self,
        store: &WorldStore,
        target: ChunkCoord,
    ) -> Result<InfiniteGenerationResult, WorldStoreError> {
        let snapshots = neighborhood_snapshots(store, target)?;
        if snapshots[4]
            .as_ref()
            .is_some_and(|snapshot| snapshot.lifecycle_flags() & FULL_LIFECYCLE == FULL_LIFECYCLE)
        {
            return Ok(InfiniteGenerationResult {
                changed_chunks: 0,
                generated_center: false,
            });
        }

        let mut preserved = Vec::with_capacity(9);
        let chunks = array::from_fn(|index| {
            let position = neighborhood_position(target, index);
            if let Some(snapshot) = snapshots[index]
                .as_ref()
                .filter(|snapshot| snapshot.lifecycle_flags() & CHUNK_LIFECYCLE_GENERATED != 0)
            {
                preserved.push(PreservedChunk::from_snapshot(snapshot));
                planes_from_snapshot(snapshot)
            } else {
                let carved = self.carver.generate(position);
                let (states, biome_ids) = carved.into_parts();
                preserved.push(PreservedChunk::fresh(&biome_ids));
                PopulationChunkPlanes {
                    generation_height_map: generation_height_map(&states),
                    states,
                    biome_ids,
                    extra_data: BTreeMap::new(),
                }
            }
        });
        let preserved: [PreservedChunk; 9] = preserved
            .try_into()
            .expect("nine population-neighborhood preservation entries");

        let mut neighborhood = PopulationNeighborhood::from_planes(target, chunks);

        self.lakes.populate(&mut neighborhood);

        self.villages.apply(&mut self.state.village, target);
        self.mineshafts.apply(&mut self.state.mineshaft, target);
        self.strongholds.apply(&mut self.state.stronghold, target);
        self.scattered.apply(&mut self.state.scattered, target);

        let mut structure_random = population_random(self.seed_bits, target);
        let _ = self.villages.post_process_with_random(
            &mut self.state.village,
            &mut neighborhood,
            target,
            &mut structure_random,
        );
        let _ = self.mineshafts.post_process_with_random(
            &mut self.state.mineshaft,
            &mut neighborhood,
            target,
            &mut structure_random,
        );
        let _ = self.strongholds.post_process_with_random(
            &mut self.state.stronghold,
            &mut neighborhood,
            target,
            &mut structure_random,
        );
        let _ = self.scattered.post_process_with_random(
            &mut self.state.scattered,
            &mut neighborhood,
            target,
            &mut structure_random,
        );
        self.monster_rooms
            .populate_with_random(&mut neighborhood, &mut structure_random);

        self.freeze.populate(&mut neighborhood);
        self.biome_decorator.decorate(&mut neighborhood);
        let finalized = self.finalizer.finalize(&mut neighborhood);
        let planes = neighborhood.into_planes();

        let mut changed_chunks = 0;
        for (index, (planes, preserved)) in planes.into_iter().zip(preserved).enumerate() {
            let position = neighborhood_position(target, index);
            let center = index == 4;
            let lifecycle_flags = if center {
                FULL_LIFECYCLE
            } else {
                preserved.lifecycle_flags | CHUNK_LIFECYCLE_GENERATED
            };
            let (height_map, sky_light, block_light) = if center {
                (
                    finalized.height_map().to_vec(),
                    finalized.sky_light().to_vec(),
                    finalized.block_light().to_vec(),
                )
            } else {
                (
                    planes.generation_height_map.clone(),
                    preserved.sky_light,
                    preserved.block_light,
                )
            };

            let import = ChunkImport {
                terrain_revision: 0,
                light_revision: 0,
                lifecycle_flags,
                states: planes.states,
                sky_light,
                block_light,
                biomes: preserved.biome_words,
                height_map,
                extra_data: population_extra_to_store(planes.extra_data),
            };
            if store.install_generated_chunk(position, import)? {
                changed_chunks += 1;
            }
        }

        Ok(InfiniteGenerationResult {
            changed_chunks,
            generated_center: true,
        })
    }
}

#[derive(Debug)]
struct PreservedChunk {
    lifecycle_flags: u8,
    sky_light: Vec<u8>,
    block_light: Vec<u8>,
    biome_words: Vec<u32>,
}

impl PreservedChunk {
    fn from_snapshot(snapshot: &ChunkSnapshot) -> Self {
        Self {
            lifecycle_flags: snapshot.lifecycle_flags(),
            sky_light: snapshot.sky_light().to_vec(),
            block_light: snapshot.block_light().to_vec(),
            biome_words: snapshot.biomes().to_vec(),
        }
    }

    fn fresh(biome_ids: &[u8]) -> Self {
        Self {
            lifecycle_flags: 0,
            sky_light: vec![0; CHUNK_NIBBLE_BYTES],
            block_light: vec![0; CHUNK_NIBBLE_BYTES],
            biome_words: biome_ids
                .iter()
                .map(|id| {
                    default_biome_word(*id).expect("generated biome id is fixed-target valid")
                })
                .collect(),
        }
    }
}

fn neighborhood_snapshots(
    store: &WorldStore,
    center: ChunkCoord,
) -> Result<[Option<ChunkSnapshot>; 9], WorldStoreError> {
    let mut snapshots = Vec::with_capacity(9);
    for index in 0..9 {
        let position = neighborhood_position(center, index);
        snapshots.push(if store.contains_chunk(position) {
            Some(store.snapshot(position)?)
        } else {
            None
        });
    }
    Ok(snapshots
        .try_into()
        .expect("nine population-neighborhood snapshot entries"))
}

fn neighborhood_position(center: ChunkCoord, index: usize) -> ChunkCoord {
    let offset_x = (index % 3) as i32 - 1;
    let offset_z = (index / 3) as i32 - 1;
    ChunkCoord::new(
        center.x().wrapping_add(offset_x),
        center.z().wrapping_add(offset_z),
    )
}

fn planes_from_snapshot(snapshot: &ChunkSnapshot) -> PopulationChunkPlanes {
    PopulationChunkPlanes {
        states: snapshot.states().to_vec(),
        biome_ids: snapshot
            .biomes()
            .iter()
            .map(|word| biome_id(*word))
            .collect(),
        generation_height_map: snapshot.height_map().to_vec(),
        extra_data: store_extra_to_population(snapshot.extra_data()),
    }
}

fn store_extra_to_population(extra: &BTreeMap<u16, u16>) -> BTreeMap<u16, u16> {
    extra
        .iter()
        .map(|(&key, &value)| (extra_key_to_linear_index(key), value))
        .collect()
}

fn population_extra_to_store(extra: BTreeMap<u16, u16>) -> BTreeMap<u16, u16> {
    extra
        .into_iter()
        .map(|(key, value)| (linear_index_to_extra_key(key), value))
        .collect()
}

const fn extra_key_to_linear_index(key: u16) -> u16 {
    let z = (key >> 12) & 0x0f;
    let x = (key >> 8) & 0x0f;
    let y = key & 0x7f;
    (y << 8) | (z << 4) | x
}

/// Resolves the fixed-target first-player spawn from BiomeSource's X/Z selection against a
/// fully generated 5x5 spawn view (PlayerChunkSource radius = CHUNK_WIDTH * 2).
pub fn resolve_overworld_initial_spawn(seed: i32) -> Result<[i32; 3], WorldStoreError> {
    let source = OverworldBiomeSource::new(seed);
    let (spawn_x, spawn_z) = source.spawn_position();
    let origin_chunk = ChunkCoord::new(spawn_x.div_euclid(16), spawn_z.div_euclid(16));

    let store = WorldStore::new();
    let mut generator = OverworldInfiniteGenerator::new(seed);

    // Match InitialChunkView's authoritative X-then-Z center order. Each center keeps native
    // cross-chunk writes resident for later centers before the spawn column is inspected.
    for dx in -2_i32..=2 {
        for dz in -2_i32..=2 {
            let position = ChunkCoord::new(
                origin_chunk.x().wrapping_add(dx),
                origin_chunk.z().wrapping_add(dz),
            );
            let _ = generator.generate_into_store(&store, position)?;
        }
    }

    resolve_overworld_spawn_from_store(&store, spawn_x, spawn_z)
}

pub fn resolve_overworld_spawn_from_store(
    store: &WorldStore,
    spawn_x: i32,
    spawn_z: i32,
) -> Result<[i32; 3], WorldStoreError> {
    // MCPE 0.15.10 Player::recheckSpawnPosition resolves the shared Y=128 sentinel with
    // BlockSource::getTopSolidBlock(x, z, true, true) while preserving BiomeSource's X/Z.
    // The later Player::fixSpawnPosition collision/liquid repair is gated to respawn state and
    // must not be applied to the initial world entry.
    let spawn_y = above_top_solid(store, spawn_x, spawn_z)?;
    Ok([spawn_x, i32::from(spawn_y), spawn_z])
}

fn above_top_solid(store: &WorldStore, world_x: i32, world_z: i32) -> Result<u8, WorldStoreError> {
    let snapshot = store.snapshot(ChunkCoord::new(
        world_x.div_euclid(16),
        world_z.div_euclid(16),
    ))?;
    let local_x = world_x.rem_euclid(16) as usize;
    let local_z = world_z.rem_euclid(16) as usize;
    for y in (0_usize..128).rev() {
        let id = snapshot.states()[(y << 8) | (local_z << 4) | local_x] >> 4;
        if is_spawn_top_solid(id) {
            return Ok((y + 1) as u8);
        }
    }
    Ok(0)
}

const fn is_spawn_top_solid(id: u16) -> bool {
    matches!(id, 8 | 9) || is_leaves(id) || material_blocks_motion(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn semantic_hash(snapshot: &ChunkSnapshot) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for value in snapshot.states() {
            for byte in value.to_le_bytes() {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
            }
        }
        for bytes in [snapshot.sky_light(), snapshot.block_light()] {
            for &byte in bytes {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
            }
        }
        for value in snapshot.biomes() {
            for byte in value.to_le_bytes() {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
            }
        }
        for &byte in snapshot.height_map() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        }
        for (&key, &value) in snapshot.extra_data() {
            for byte in key.to_le_bytes().into_iter().chain(value.to_le_bytes()) {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
            }
        }
        hash
    }

    #[test]
    fn mama_moose_initial_spawn_resolves_above_generated_surface() {
        let spawn = resolve_overworld_initial_spawn(-1_385_905_961).unwrap();
        assert_eq!(spawn, [4, 63, 4]);

        assert_eq!(resolve_overworld_initial_spawn(0).unwrap(), [820, 72, 4]);
    }

    #[test]
    fn aggregate_structure_state_round_trips() {
        let state = OverworldInfiniteState::new();
        let encoded = state.encode();
        let decoded = OverworldInfiniteState::decode(&encoded).expect("valid aggregate state");
        assert_eq!(decoded, state);
        assert_eq!(decoded.encode(), encoded);

        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(OverworldInfiniteState::decode(&trailing).is_none());
        assert!(OverworldInfiniteState::decode(&encoded[..encoded.len() - 1]).is_none());
    }

    #[test]
    fn full_composer_marks_center_complete_and_keeps_neighbors_generated() {
        let store = WorldStore::new();
        let target = ChunkCoord::new(0, 0);
        let mut generator = OverworldInfiniteGenerator::new(0);

        let result = generator.generate_into_store(&store, target).unwrap();
        assert!(result.generated_center());
        assert!(result.changed_chunks() > 0);
        assert_eq!(store.lifecycle_flags(target).unwrap(), FULL_LIFECYCLE);
        for index in 0..9 {
            if index == 4 {
                continue;
            }
            let position = neighborhood_position(target, index);
            assert_eq!(
                store.lifecycle_flags(position).unwrap(),
                CHUNK_LIFECYCLE_GENERATED
            );
        }

        let center = store.snapshot(target).unwrap();
        assert_ne!(semantic_hash(&center), 0);
        assert!(center.height_map().iter().any(|height| *height > 0));
        assert!(center.sky_light().iter().any(|byte| *byte != 0));

        let replay = generator.generate_into_store(&store, target).unwrap();
        assert!(!replay.generated_center());
        assert_eq!(replay.changed_chunks(), 0);
    }

    #[test]
    fn representative_whole_pipeline_fixtures_are_stable() {
        // These are composition-regression fixtures after the exact 0.15.10 biome graph is
        // selected. Independent target parity for generated block planes is tracked separately;
        // this test protects stage composition/lifecycle from accidental drift.
        let fixtures = [
            (
                "origin-river",
                0,
                ChunkCoord::new(0, 0),
                7,
                0x863d_9ed3_9028_9419_u64,
            ),
            (
                "cold-snow",
                42,
                ChunkCoord::new(-28, -18),
                12,
                0x7926_aba4_b770_6b3f_u64,
            ),
            (
                "savanna",
                0,
                ChunkCoord::new(-128, -128),
                35,
                0xc251_d7ce_b68b_f3a7_u64,
            ),
            (
                "desert-pyramid",
                0,
                ChunkCoord::new(981, -1016),
                2,
                0xef24_9a32_d206_c5df_u64,
            ),
            (
                "negative-seed-negative-chunk",
                -1,
                ChunkCoord::new(-78, -128),
                4,
                0x1eaa_36db_4261_f97b_u64,
            ),
        ];

        for (name, seed, target, expected_biome, expected_hash) in fixtures {
            let store = WorldStore::new();
            let mut generator = OverworldInfiniteGenerator::new(seed);
            generator.generate_into_store(&store, target).unwrap();
            let snapshot = store.snapshot(target).unwrap();

            assert_eq!(
                biome_id(snapshot.biomes()[8 + 8 * 16]),
                expected_biome,
                "fixture={name}",
            );
            assert_eq!(semantic_hash(&snapshot), expected_hash, "fixture={name}");
            assert_eq!(snapshot.lifecycle_flags(), FULL_LIFECYCLE, "fixture={name}");

            if name == "cold-snow" {
                let snow = snapshot
                    .states()
                    .iter()
                    .filter(|state| (**state >> 4) == 78)
                    .count();
                let ice = snapshot
                    .states()
                    .iter()
                    .filter(|state| (**state >> 4) == 79)
                    .count();
                assert_eq!(snow, 66);
                assert_eq!(ice, 0);
            }
            if name == "desert-pyramid" {
                assert!(!generator.state.scattered.is_empty());
            }
        }
    }

    #[test]
    fn resident_generated_neighbor_is_authoritative_input() {
        let store = WorldStore::new();
        let first = ChunkCoord::new(0, 0);
        let second = ChunkCoord::new(1, 0);
        let mut generator = OverworldInfiniteGenerator::new(-1);

        generator.generate_into_store(&store, first).unwrap();
        let before = store.snapshot(second).unwrap();
        assert_eq!(
            before.lifecycle_flags() & CHUNK_LIFECYCLE_GENERATED,
            CHUNK_LIFECYCLE_GENERATED
        );

        generator.generate_into_store(&store, second).unwrap();
        let after = store.snapshot(second).unwrap();
        assert_eq!(after.lifecycle_flags(), FULL_LIFECYCLE);
        assert!(after.terrain_revision() >= before.terrain_revision());
    }
}
