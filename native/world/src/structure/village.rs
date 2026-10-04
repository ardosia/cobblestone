use crate::ChunkCoord;
use crate::biome_source::OverworldBiomeSource;
use crate::terrain_shape::noise::MtRandom;

use super::village_plan::{VillagePlan, VillageStyle};
use super::{structure_source_chunks, structure_source_random};

const TOWN_SPACING: i32 = 40;
const MIN_TOWN_SEPARATION: i32 = 12;
const VILLAGE_SALT: i64 = 10_387_312;
const X_SEED_SCALE: i64 = 341_873_128_712;
const Z_SEED_SCALE: i64 = 132_897_987_541;
const VILLAGE_SOURCE_RADIUS: i32 = 4;

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

    pub(crate) fn is_feature_chunk(&self, chunk: ChunkCoord) -> bool {
        let (candidate, _) = candidate_with_random(self.seed, chunk);
        candidate && self.biome_allowed(chunk)
    }

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

struct CandidateCheck {
    candidate: bool,
    random: MtRandom,
    seed: u32,
}

fn candidate_check(seed: u32, chunk: ChunkCoord) -> CandidateCheck {
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

    let mut random = MtRandom::new(candidate_seed);
    center_x = center_x
        .wrapping_mul(TOWN_SPACING)
        .wrapping_add(random.next_int((TOWN_SPACING - MIN_TOWN_SEPARATION) as u32) as i32);
    center_z = center_z
        .wrapping_mul(TOWN_SPACING)
        .wrapping_add(random.next_int((TOWN_SPACING - MIN_TOWN_SEPARATION) as u32) as i32);

    CandidateCheck {
        candidate: chunk.x() == center_x && chunk.z() == center_z,
        random,
        seed: candidate_seed,
    }
}

fn candidate_with_random(seed: u32, chunk: ChunkCoord) -> (bool, MtRandom) {
    let check = candidate_check(seed, chunk);
    (check.candidate, check.random)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structure::village_plan::VillagePieceKind;

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
            assert!(
                check.candidate,
                "fixture must be a candidate seed={seed} {x}:{z}"
            );
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
