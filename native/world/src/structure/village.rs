use crate::ChunkCoord;
use crate::biome_source::OverworldBiomeSource;
use crate::population::{PopulationNeighborhood, population_seed};
use crate::terrain_shape::noise::MtRandom;

use super::village_place::VillagePostProcessor;
use super::village_plan::{SemanticCursor, VillagePlan, VillageStyle};
use super::{StructureStartCache, chunk_hash, structure_source_chunks, structure_source_random};

const TOWN_SPACING: i32 = 40;
const MIN_TOWN_SEPARATION: i32 = 12;
const VILLAGE_SALT: i64 = 10_387_312;
const X_SEED_SCALE: i64 = 341_873_128_712;
const Z_SEED_SCALE: i64 = 132_897_987_541;
const VILLAGE_SOURCE_RADIUS: i32 = 4;

// GNU libstdc++ 4.9's default unordered_map starts at 11 buckets and doubles through
// this prime-policy sequence. Village state rejects more than 4096 starts on reload,
// so 7517 is the last bucket count reachable by any durable state.
const GNU49_VILLAGE_BUCKET_COUNTS: &[usize] = &[11, 23, 47, 97, 199, 409, 823, 1741, 3739, 7517];
const GNU49_EMPTY_BUCKET: usize = usize::MAX;
const GNU49_BEFORE_BEGIN: usize = usize::MAX - 1;

const PLAINS: u8 = 1;
const DESERT: u8 = 2;
const ICE_FLATS: u8 = 12;
const SAVANNA: u8 = 35;

pub(crate) struct VillageFeature {
    seed: u32,
    biomes: OverworldBiomeSource,
}

impl VillageFeature {
    pub(crate) fn new(seed: i32) -> Self {
        Self {
            seed: u32::from_ne_bytes(seed.to_ne_bytes()),
            biomes: OverworldBiomeSource::new(seed),
        }
    }

    #[cfg(test)]
    pub(crate) fn is_feature_chunk(&self, chunk: ChunkCoord) -> bool {
        let mut random = MtRandom::new(0);
        self.is_feature_chunk_with_random(chunk, &mut random)
    }

    pub(crate) fn is_feature_chunk_with_random(
        &self,
        chunk: ChunkCoord,
        random: &mut MtRandom,
    ) -> bool {
        let (candidate, _) = candidate_into_random(self.seed, chunk, random);
        candidate && self.biome_allowed(chunk)
    }

    #[cfg(test)]
    pub(crate) fn discover_sources_for_target(&self, target: ChunkCoord) -> Vec<ChunkCoord> {
        structure_source_chunks(target, VILLAGE_SOURCE_RADIUS)
            .into_iter()
            .filter(|source| {
                // StructureFeature::addFeature clears one key from the per-source LargeFeature
                // stream before Village reseeds for its own spacing check.
                let mut source_random = structure_source_random(self.seed, *source);
                let _ = source_random.next_positive_int();
                self.is_feature_chunk(*source)
            })
            .collect()
    }

    pub(crate) fn plan(&self, source: ChunkCoord) -> Option<VillagePlan> {
        let check = candidate_check(self.seed, source);
        if !check.candidate || !self.biome_allowed(source) {
            return None;
        }
        let west = source.x().wrapping_mul(16).wrapping_add(2);
        let north = source.z().wrapping_mul(16).wrapping_add(2);
        let biome = self.biomes.biome_ids(west, north, 1, 1)[0];
        let style = match biome {
            2 | 17 => VillageStyle::Desert,
            35 | 36 | 163 | 164 => VillageStyle::Savanna,
            5 | 12 | 30 | 31 | 158 => VillageStyle::Taiga,
            _ => VillageStyle::Plains,
        };
        Some(VillagePlan::generate(
            source,
            check.random,
            check.seed,
            style,
        ))
    }

    #[cfg(test)]
    pub(crate) fn random_after_candidate_check(&self, chunk: ChunkCoord) -> Option<MtRandom> {
        let check = candidate_check(self.seed, chunk);
        (check.candidate && self.biome_allowed(chunk)).then_some(check.random)
    }

    fn biome_allowed(&self, chunk: ChunkCoord) -> bool {
        let block_x = chunk.x().wrapping_mul(16).wrapping_add(8);
        let block_z = chunk.z().wrapping_mul(16).wrapping_add(8);
        let raw_x = block_x >> 2;
        let raw_z = block_z >> 2;
        matches!(
            self.biomes.raw_biome_ids(raw_x, raw_z, 1, 1)[0],
            PLAINS | DESERT | SAVANNA | ICE_FLATS
        )
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct VillageStructureState {
    starts: StructureStartCache<VillagePlan>,
}

impl VillageStructureState {
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
        bytes.extend_from_slice(b"CVS1");
        bytes.extend_from_slice(&(self.starts.len() as u32).to_le_bytes());
        for start in self.starts.iter() {
            start.encode_semantics(&mut bytes);
        }
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 8 || &bytes[..4] != b"CVS1" {
            return None;
        }
        let count = u32::from_le_bytes(bytes[4..8].try_into().ok()?) as usize;
        if count > 4096 {
            return None;
        }
        let mut cursor = SemanticCursor::new(&bytes[8..]);
        let mut starts = Vec::with_capacity(count);
        for _ in 0..count {
            starts.push(VillagePlan::decode_semantics(&mut cursor)?);
        }
        cursor.is_finished().then_some(Self {
            starts: StructureStartCache::from_starts(starts),
        })
    }
}

pub struct OverworldVillageStructures {
    seed: u32,
    feature: VillageFeature,
}

impl OverworldVillageStructures {
    pub fn new(seed: i32) -> Self {
        Self {
            seed: u32::from_ne_bytes(seed.to_ne_bytes()),
            feature: VillageFeature::new(seed),
        }
    }

    pub fn apply(&self, state: &mut VillageStructureState, target: ChunkCoord) {
        for source in structure_source_chunks(target, VILLAGE_SOURCE_RADIUS) {
            let mut source_random = structure_source_random(self.seed, source);
            let _ = source_random.next_positive_int();

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
        state: &mut VillageStructureState,
        neighborhood: &mut PopulationNeighborhood,
        target: ChunkCoord,
    ) -> bool {
        let mut random = MtRandom::new(population_seed(self.seed, target));
        self.post_process_with_random(state, neighborhood, target, &mut random)
    }

    pub(crate) fn post_process_with_random(
        &self,
        state: &mut VillageStructureState,
        neighborhood: &mut PopulationNeighborhood,
        target: ChunkCoord,
        random: &mut MtRandom,
    ) -> bool {
        let processor = VillagePostProcessor::new(population_seed(self.seed, target));
        let target_order = target_structure_map_iteration_order(&state.starts);
        let mut changed = false;
        for index in target_order {
            let start = state
                .starts
                .get_mut(index)
                .expect("target Village start order contains valid indices");
            changed |= processor.process_start(start, neighborhood, target, random);
        }
        changed
    }
}

fn target_structure_map_iteration_order(starts: &StructureStartCache<VillagePlan>) -> Vec<usize> {
    let sources: Vec<_> = starts.iter().map(|start| start.core.source()).collect();
    gnu49_unordered_map_iteration_order(&sources)
}

fn gnu49_unordered_map_iteration_order(sources: &[ChunkCoord]) -> Vec<usize> {
    if sources.is_empty() {
        return Vec::new();
    }

    let mut bucket_count = GNU49_VILLAGE_BUCKET_COUNTS[0];
    let mut next_resize = bucket_count;
    let mut bucket_before = vec![GNU49_EMPTY_BUCKET; bucket_count];
    let mut next = vec![None; sources.len()];
    let mut head = None;

    for index in 0..sources.len() {
        let element_count_after = index + 1;
        if element_count_after >= next_resize {
            let min_buckets = element_count_after;
            if min_buckets >= bucket_count {
                let requested = (min_buckets + 1).max(bucket_count * 2);
                let new_bucket_count = GNU49_VILLAGE_BUCKET_COUNTS
                    .iter()
                    .copied()
                    .find(|count| *count >= requested)
                    .expect("durable Village start count fits GNU 4.9 bucket table");

                let old_order = linked_iteration_order(head, &next);
                bucket_count = new_bucket_count;
                next_resize = bucket_count;
                bucket_before = vec![GNU49_EMPTY_BUCKET; bucket_count];
                head = None;
                for old_index in old_order {
                    next[old_index] = None;
                    gnu49_insert_node(
                        sources,
                        old_index,
                        bucket_count,
                        &mut bucket_before,
                        &mut next,
                        &mut head,
                    );
                }
            } else {
                next_resize = bucket_count;
            }
        }

        gnu49_insert_node(
            sources,
            index,
            bucket_count,
            &mut bucket_before,
            &mut next,
            &mut head,
        );
    }

    linked_iteration_order(head, &next)
}

fn gnu49_insert_node(
    sources: &[ChunkCoord],
    index: usize,
    bucket_count: usize,
    bucket_before: &mut [usize],
    next: &mut [Option<usize>],
    head: &mut Option<usize>,
) {
    let bucket = target_chunk_bucket(sources[index], bucket_count);
    let before = bucket_before[bucket];

    if before == GNU49_EMPTY_BUCKET {
        next[index] = *head;
        if let Some(old_head) = *head {
            let old_bucket = target_chunk_bucket(sources[old_head], bucket_count);
            debug_assert_eq!(bucket_before[old_bucket], GNU49_BEFORE_BEGIN);
            bucket_before[old_bucket] = index;
        }
        *head = Some(index);
        bucket_before[bucket] = GNU49_BEFORE_BEGIN;
        return;
    }

    if before == GNU49_BEFORE_BEGIN {
        next[index] = *head;
        *head = Some(index);
        return;
    }

    next[index] = next[before];
    next[before] = Some(index);
}

fn target_chunk_bucket(source: ChunkCoord, bucket_count: usize) -> usize {
    (chunk_hash(source.x(), source.z()) as u32 as usize) % bucket_count
}

fn linked_iteration_order(mut head: Option<usize>, next: &[Option<usize>]) -> Vec<usize> {
    let mut order = Vec::with_capacity(next.len());
    while let Some(index) = head {
        order.push(index);
        head = next[index];
    }
    order
}

struct CandidateCheck {
    candidate: bool,
    random: MtRandom,
    seed: u32,
}

fn candidate_check(seed: u32, chunk: ChunkCoord) -> CandidateCheck {
    let mut random = MtRandom::new(0);
    let (candidate, candidate_seed) = candidate_into_random(seed, chunk, &mut random);
    CandidateCheck {
        candidate,
        random,
        seed: candidate_seed,
    }
}

fn candidate_into_random(seed: u32, chunk: ChunkCoord, random: &mut MtRandom) -> (bool, u32) {
    let mut adjusted_x = chunk.x();
    let mut adjusted_z = chunk.z();
    if adjusted_x < 0 {
        adjusted_x = adjusted_x.wrapping_sub(TOWN_SPACING - 1);
    }
    if adjusted_z < 0 {
        adjusted_z = adjusted_z.wrapping_sub(TOWN_SPACING - 1);
    }

    let mut center_x = adjusted_x / TOWN_SPACING;
    let mut center_z = adjusted_z / TOWN_SPACING;
    let mixed = i64::from(adjusted_x)
        .wrapping_mul(X_SEED_SCALE)
        .wrapping_add(i64::from(adjusted_z).wrapping_mul(Z_SEED_SCALE))
        .wrapping_add(i64::from(seed))
        .wrapping_add(VILLAGE_SALT);
    let candidate_seed = mixed as u32;

    random.reseed(candidate_seed);
    // MCPE 0.15.10 VillageFeature::isFeatureChunk takes each raw MT output modulo 28.
    // The target Thumb code shifts the raw value while strength-reducing division by 28,
    // but computes the remainder by subtracting quotient * 28 from the original output.
    // Do not mistake that compiler sequence for (genrand_int32() >> 2) % 28.
    let bound = (TOWN_SPACING - MIN_TOWN_SEPARATION) as u32;
    center_x = center_x
        .wrapping_mul(TOWN_SPACING)
        .wrapping_add((random.next_u32() % bound) as i32);
    center_z = center_z
        .wrapping_mul(TOWN_SPACING)
        .wrapping_add((random.next_u32() % bound) as i32);

    (
        chunk.x() == center_x && chunk.z() == center_z,
        candidate_seed,
    )
}

#[cfg(test)]
fn candidate_with_random(seed: u32, chunk: ChunkCoord) -> (bool, MtRandom) {
    let check = candidate_check(seed, chunk);
    (check.candidate, check.random)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::population::{PopulationNeighborhood, population_seed, state};
    use crate::structure::village_place::VillagePostProcessor;
    use crate::structure::village_plan::VillagePieceKind;

    fn first_allowed_source(feature: &VillageFeature) -> ChunkCoord {
        (-512..=512)
            .flat_map(|z| (-512..=512).map(move |x| ChunkCoord::new(x, z)))
            .find(|chunk| feature.is_feature_chunk(*chunk))
            .expect("expected fixed-target allowed Village source")
    }

    fn flat_population(center: ChunkCoord, biome: u8) -> PopulationNeighborhood {
        let mut neighborhood = PopulationNeighborhood::filled(center, state(0, 0), biome);
        let origin_x = center.x().wrapping_mul(16);
        let origin_z = center.z().wrapping_mul(16);
        for z in origin_z..origin_z.wrapping_add(16) {
            for x in origin_x..origin_x.wrapping_add(16) {
                for y in 0..=62 {
                    neighborhood.set_state(x, y, z, state(1, 0));
                }
                neighborhood.set_state(x, 63, z, state(2, 0));
            }
        }
        neighborhood
    }

    fn hash_center(neighborhood: &PopulationNeighborhood) -> u64 {
        neighborhood
            .center_states()
            .iter()
            .fold(0xcbf2_9ce4_8422_2325_u64, |mut hash, value| {
                for byte in value.to_le_bytes() {
                    hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
                }
                hash
            })
    }

    #[test]
    fn mama_moose_origin_village_sources() {
        let feature = VillageFeature::new(-1_385_905_961);
        // The real 0.15.10 offline world contains target Village Well signatures whose
        // StartPiece origins map back to these three nearby source chunks.
        for source in [
            ChunkCoord::new(2, 3),
            ChunkCoord::new(1, 6),
            ChunkCoord::new(0, 9),
        ] {
            assert!(candidate_with_random(feature.seed, source).0);
            assert!(feature.biome_allowed(source));
            assert!(
                feature
                    .discover_sources_for_target(source)
                    .contains(&source)
            );
        }

        // These were false positives from the old misread (raw >> 2) % 28 oracle.
        for source in [ChunkCoord::new(1, 8), ChunkCoord::new(1, 17)] {
            assert!(!candidate_with_random(feature.seed, source).0);
        }
    }

    #[test]
    fn gnu49_structure_map_order_matches_mama_moose_offline_overwrites() {
        let sources = [
            ChunkCoord::new(2, 3),
            ChunkCoord::new(1, 6),
            ChunkCoord::new(0, 9),
        ];
        assert_eq!(gnu49_unordered_map_iteration_order(&sources), [2, 1, 0]);

        let same_bucket = [
            ChunkCoord::new(0, 0),
            ChunkCoord::new(0, 11),
            ChunkCoord::new(0, 1),
        ];
        assert_eq!(gnu49_unordered_map_iteration_order(&same_bucket), [2, 1, 0]);

        let rehash: Vec<_> = (0..11).map(|z| ChunkCoord::new(0, z)).collect();
        assert_eq!(
            gnu49_unordered_map_iteration_order(&rehash),
            [10, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
        );
    }

    #[test]
    fn mama_moose_cross_start_overwrites_match_offline_precedence() {
        let seed = -1_385_905_961;
        let runtime = OverworldVillageStructures::new(seed);
        let offline_sources = [
            ChunkCoord::new(2, 3),
            ChunkCoord::new(1, 6),
            ChunkCoord::new(0, 9),
        ];

        let mut farm_house_state = VillageStructureState::new();
        for source in offline_sources {
            farm_house_state.starts.push(
                runtime
                    .feature
                    .plan(source)
                    .expect("offline-backed Village source"),
            );
        }

        // Real 0.15.10 output at x50..55,z80 is the (2,3) BookHouse stair roof even
        // though the (1,6) Farmland bounds overlap the same columns. Synthetic flat
        // terrain shifts Y, but the winning block family must stay the house.
        let farm_house_target = ChunkCoord::new(3, 5);
        let mut farm_house = flat_population(farm_house_target, PLAINS);
        assert!(runtime.post_process(&mut farm_house_state, &mut farm_house, farm_house_target));
        for x in 50..=55 {
            let top_y = (0..128)
                .rev()
                .find(|y| farm_house.block_id(x, *y, 80) != 0)
                .expect("flat column stays non-air");
            assert_eq!(
                farm_house.block_id(x, top_y, 80),
                53,
                "Farmland incorrectly won the cross-start overlap at x={x}",
            );
        }

        let mut road_house_state = VillageStructureState::new();
        for source in offline_sources {
            road_house_state.starts.push(
                runtime
                    .feature
                    .plan(source)
                    .expect("offline-backed Village source"),
            );
        }

        // The real offline world has GrassPath as the top solid block across this
        // (1,6) StraightRoad / (0,9) TwoRoomHouse overlap. The road therefore runs
        // after the house and intentionally paints the house roof/top surface.
        let road_house_target = ChunkCoord::new(0, 8);
        let mut road_house = flat_population(road_house_target, PLAINS);
        assert!(runtime.post_process(&mut road_house_state, &mut road_house, road_house_target));
        for z in 128..=133 {
            for x in 11..=13 {
                let top_y = (0..128)
                    .rev()
                    .find(|y| road_house.block_id(x, *y, z) != 0)
                    .expect("flat column stays non-air");
                assert_eq!(
                    road_house.block_id(x, top_y, z),
                    198,
                    "target StraightRoad did not win the cross-start overlap at {x},{z}",
                );
            }
        }
    }

    #[test]
    fn independent_house_recipe_fixtures_match_without_roads() {
        // Standalone restored-source C++ oracle: target MT/topology and the fixed
        // Well/SimpleHouse/SmallHut recipes over synthetic y=63 flat terrain. That later
        // source comments out StraightRoad painting, unlike the actual 0.15.10 APK; exclude
        // roads here to preserve independent coverage of the unchanged house recipes.
        let source = ChunkCoord::new(-221, -239);
        let check = candidate_check(0, source);
        // Planner oracle: candidate qualification is covered separately. The target candidate
        // check consumes the same two raw MT outputs before VillageStart regardless of whether
        // this synthetic source wins the spacing comparison.
        let base = VillagePlan::generate(source, check.random, check.seed, VillageStyle::Plains);
        assert!(!base.abandoned);

        let fixtures = [
            (VillageStyle::Plains, 1, 0x14a2_5f95_7058_fce5_u64),
            (VillageStyle::Desert, 2, 0x0362_c218_e146_0b15_u64),
            (VillageStyle::Savanna, 35, 0x545e_1ccd_257f_f4c5_u64),
            (VillageStyle::Taiga, 12, 0x979e_dbe1_7fca_afd5_u64),
        ];

        for (style, biome, expected) in fixtures {
            let mut plan = base.clone();
            plan.style = style;
            plan.pieces
                .retain(|piece| piece.kind != VillagePieceKind::StraightRoad);
            let mut neighborhood = flat_population(source, biome);
            let post_seed = population_seed(0, source);
            let mut random = MtRandom::new(post_seed);
            assert!(VillagePostProcessor::new(post_seed).process_start(
                &mut plan,
                &mut neighborhood,
                source,
                &mut random,
            ));
            assert_eq!(hash_center(&neighborhood), expected, "style={style:?}");
        }
    }

    #[test]
    fn independent_small_hut_chunk_fixture_matches() {
        let source = ChunkCoord::new(-114, -196);
        let target = ChunkCoord::new(-116, -197);
        let check = candidate_check(0, source);
        let mut plan =
            VillagePlan::generate(source, check.random, check.seed, VillageStyle::Plains);
        assert!(!plan.abandoned);

        let mut neighborhood = flat_population(target, 1);
        let post_seed = population_seed(0, target);
        let mut random = MtRandom::new(post_seed);
        assert!(VillagePostProcessor::new(post_seed).process_start(
            &mut plan,
            &mut neighborhood,
            target,
            &mut random,
        ));
        assert_eq!(hash_center(&neighborhood), 0x86e0_2e85_e732_8f25);
    }

    #[test]
    fn independent_abandoned_simple_house_chunk_fixture_matches() {
        // This target chunk intersects only one abandoned SimpleHouse (roads are no-op), making
        // mossy-selector/pass-by-value RNG behavior independently observable.
        let source = ChunkCoord::new(-384, -508);
        let target = ChunkCoord::new(-387, -509);
        let check = candidate_check(0, source);
        let mut plan =
            VillagePlan::generate(source, check.random, check.seed, VillageStyle::Plains);
        assert!(plan.abandoned);

        let mut neighborhood = flat_population(target, 1);
        let post_seed = population_seed(0, target);
        let mut random = MtRandom::new(post_seed);
        assert!(VillagePostProcessor::new(post_seed).process_start(
            &mut plan,
            &mut neighborhood,
            target,
            &mut random,
        ));
        assert_eq!(hash_center(&neighborhood), 0xc86e_dfd0_2438_c415);
    }

    #[test]
    fn structure_state_apply_is_idempotent_and_round_trips_durable_piece_state() {
        let runtime = OverworldVillageStructures::new(0);
        let source = first_allowed_source(&runtime.feature);

        let mut state = VillageStructureState::new();
        runtime.apply(&mut state, source);
        assert!(!state.is_empty());
        let count = state.len();

        runtime.apply(&mut state, source);
        assert_eq!(
            state.len(),
            count,
            "repeated apply duplicated a cached start"
        );

        let mut neighborhood = flat_population(source, 1);
        assert!(runtime.post_process(&mut state, &mut neighborhood, source));
        let once = hash_center(&neighborhood);

        assert!(
            !runtime.post_process(&mut state, &mut neighborhood, source),
            "generated chunk bookkeeping did not suppress duplicate post-process",
        );
        assert_eq!(hash_center(&neighborhood), once);

        let encoded = state.encode();
        let decoded = VillageStructureState::decode(&encoded).expect("valid village state");
        assert_eq!(
            decoded.encode(),
            encoded,
            "durable structure semantics did not round-trip byte-for-byte",
        );
        assert_eq!(decoded.len(), state.len());

        // Target StructureStart tags do not serialize generatedChunkPositions. Reload therefore
        // restores durable starts/pieces while resetting per-chunk post-process bookkeeping.
        // Compare that restored state on the same fresh population input; persisted populated
        // chunks are not legitimately post-processed a second time after reload.
        let mut reloaded = decoded;
        let mut replay = flat_population(source, 1);
        assert!(runtime.post_process(&mut reloaded, &mut replay, source));
        assert_eq!(
            hash_center(&replay),
            once,
            "deterministic replay after target-compatible reload changed block output",
        );
    }

    #[test]
    fn structure_state_codec_rejects_truncated_or_trailing_data() {
        let runtime = OverworldVillageStructures::new(0);
        let source = first_allowed_source(&runtime.feature);
        let mut state = VillageStructureState::new();
        runtime.apply(&mut state, source);
        let encoded = state.encode();

        assert!(VillageStructureState::decode(&encoded[..encoded.len() - 1]).is_none());

        let mut trailing = encoded;
        trailing.push(0);
        assert!(VillageStructureState::decode(&trailing).is_none());
    }

    #[test]
    fn independent_candidate_locator_fixtures_match_target_bug_compatibility() {
        // Standalone C++ oracle reproducing VillageFeature spacing/seed math only.
        let fixtures: &[(i32, &[(i32, i32)])] = &[
            (
                0,
                &[
                    (139, -158),
                    (-112, -155),
                    (42, -147),
                    (-17, -141),
                    (66, -141),
                    (122, -141),
                    (25, -134),
                    (-94, -133),
                ],
            ),
            (
                -1,
                &[
                    (-79, -160),
                    (92, -158),
                    (99, -156),
                    (-107, -151),
                    (-36, -150),
                    (17, -150),
                    (21, -150),
                    (-107, -136),
                ],
            ),
            (
                i32::MIN,
                &[
                    (8, -160),
                    (-55, -153),
                    (-149, -152),
                    (21, -150),
                    (-156, -147),
                    (59, -147),
                    (-119, -140),
                    (-24, -133),
                ],
            ),
        ];

        for (seed, expected_prefix) in fixtures {
            let bits = u32::from_ne_bytes(seed.to_ne_bytes());
            let mut actual = Vec::new();
            'outer: for z in -160..=160 {
                for x in -160..=160 {
                    if candidate_with_random(bits, ChunkCoord::new(x, z)).0 {
                        actual.push((x, z));
                        if actual.len() == expected_prefix.len() {
                            break 'outer;
                        }
                    }
                }
            }
            assert_eq!(&actual, expected_prefix, "seed={seed}");
        }
    }

    #[test]
    fn biome_gate_only_accepts_fixed_target_village_biomes() {
        for seed in [0, 1, -1, i32::MIN, 0x1234_5678] {
            let feature = VillageFeature::new(seed);
            for z in -96..=96 {
                for x in -96..=96 {
                    let chunk = ChunkCoord::new(x, z);
                    if feature.is_feature_chunk(chunk) {
                        let block_x = x.wrapping_mul(16).wrapping_add(8);
                        let block_z = z.wrapping_mul(16).wrapping_add(8);
                        let raw = feature
                            .biomes
                            .raw_biome_ids(block_x >> 2, block_z >> 2, 1, 1)[0];
                        assert!(matches!(raw, PLAINS | DESERT | SAVANNA | ICE_FLATS));
                    }
                }
            }
        }
    }

    #[test]
    fn village_source_scan_uses_radius_four_and_preserves_candidate_rng_tail() {
        let feature = VillageFeature::new(0);
        let mut selected = None;
        for z in -256..=256 {
            for x in -256..=256 {
                let chunk = ChunkCoord::new(x, z);
                if feature.is_feature_chunk(chunk) {
                    selected = Some(chunk);
                    break;
                }
            }
            if selected.is_some() {
                break;
            }
        }

        let source = selected.expect("expected an allowed-biome village source");
        let target = ChunkCoord::new(source.x().wrapping_add(4), source.z());
        assert!(
            feature
                .discover_sources_for_target(target)
                .contains(&source)
        );

        let mut first = feature
            .random_after_candidate_check(source)
            .expect("candidate random tail");
        let mut second = feature
            .random_after_candidate_check(source)
            .expect("candidate random tail");
        for _ in 0..16 {
            assert_eq!(first.next_u32(), second.next_u32());
        }
    }
    #[test]
    fn independent_full_topology_fixtures_match() {
        // Standalone C++ oracle reproduces the fixed-target MT, candidate tail, piece weights,
        // collision walk, constructor-side RNG, road expansion, crop selections, and abandoned
        // flag without consuming Rust output.
        let fixtures = [
            (0, 139, -158, 0xbacd_f5a1_588f_6fde_u64, 34, 9, false),
            (0, -112, -155, 0x03aa_adc0_9c5e_6e51_u64, 19, 6, false),
            (-1, -79, -160, 0xd306_f1af_bb79_6a37_u64, 29, 9, false),
            (i32::MIN, 8, -160, 0x744b_11ce_ea77_d781_u64, 27, 9, false),
            (
                0x1234_5678,
                20,
                -160,
                0x699f_78a0_0143_c716_u64,
                35,
                11,
                false,
            ),
            (0, -384, -508, 0x6cf1_735b_a1a3_15ec_u64, 28, 10, true),
        ];

        for (seed, x, z, expected_hash, expected_pieces, expected_roads, abandoned) in fixtures {
            let bits = u32::from_ne_bytes(seed.to_ne_bytes());
            let source = ChunkCoord::new(x, z);
            let check = candidate_check(bits, source);
            let plan =
                VillagePlan::generate(source, check.random, check.seed, VillageStyle::Plains);
            assert_eq!(plan.hash_topology(), expected_hash, "seed={seed} {x}:{z}");
            assert_eq!(plan.pieces.len(), expected_pieces, "seed={seed} {x}:{z}");
            assert_eq!(
                plan.pieces
                    .iter()
                    .filter(|piece| piece.kind == VillagePieceKind::StraightRoad)
                    .count(),
                expected_roads,
                "seed={seed} {x}:{z}",
            );
            assert_eq!(plan.abandoned, abandoned, "seed={seed} {x}:{z}");
        }
    }

    #[test]
    fn allowed_candidate_builds_a_complete_village_plan() {
        let feature = VillageFeature::new(0);
        let source = (-512..=512)
            .flat_map(|z| (-512..=512).map(move |x| ChunkCoord::new(x, z)))
            .find(|chunk| feature.is_feature_chunk(*chunk))
            .expect("expected village source");
        let plan = feature.plan(source).expect("allowed village plan");
        assert_eq!(plan.core.source(), source);
        assert!(!plan.pieces.is_empty());
        assert!(plan.core.bounds().x_span() > 6 || plan.core.bounds().z_span() > 6);
    }
}
