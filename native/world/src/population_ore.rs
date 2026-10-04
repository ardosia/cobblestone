use crate::population::{PopulationNeighborhood, population_random, state};
use crate::terrain_shape::noise::MtRandom;

const PI: f32 = std::f32::consts::PI;

const STONE_ID: u16 = 1;
const DIRT_ID: u16 = 3;
const GRAVEL_ID: u16 = 13;
const GOLD_ORE_ID: u16 = 14;
const IRON_ORE_ID: u16 = 15;
const COAL_ORE_ID: u16 = 16;
const LAPIS_ORE_ID: u16 = 21;
const DIAMOND_ORE_ID: u16 = 56;
const REDSTONE_ORE_ID: u16 = 73;

const STONE_GRANITE: u8 = 1;
const STONE_DIORITE: u8 = 3;
const STONE_ANDESITE: u8 = 5;

/// Exact fixed-target common BiomeDecorator::decorateOres stage.
///
/// Population operates on a 3x3 chunk neighborhood. Veins originating in the center chunk may
/// legitimately write into neighboring chunks, so this stage returns the complete mutated
/// neighborhood instead of silently clipping those writes to the center.
pub struct OverworldOreDecorator {
    seed: u32,
}

impl OverworldOreDecorator {
    pub fn new(seed: i32) -> Self {
        Self {
            seed: u32::from_ne_bytes(seed.to_ne_bytes()),
        }
    }

    pub fn decorate(&self, neighborhood: &mut PopulationNeighborhood) {
        let center = neighborhood.center();
        let mut random = population_random(self.seed, center);
        self.decorate_with_random(neighborhood, &mut random);
    }

    pub(crate) fn decorate_with_random(
        &self,
        neighborhood: &mut PopulationNeighborhood,
        random: &mut MtRandom,
    ) {
        let mesa = is_mesa_biome(neighborhood.center_biome_ids()[15 + 15 * 16]);

        let _ = decorate_common_ores(neighborhood, random);
        if mesa {
            decorate_depth_span(
                neighborhood,
                random,
                20,
                OreSpec::new(GOLD_ORE_ID, 0, 9),
                32,
                80,
            );
        }
    }
}

fn decorate_common_ores(neighborhood: &mut PopulationNeighborhood, random: &mut MtRandom) -> bool {
    decorate_depth_span(
        neighborhood,
        random,
        10,
        OreSpec::new(DIRT_ID, 0, 33),
        0,
        128,
    );
    decorate_depth_span(
        neighborhood,
        random,
        8,
        OreSpec::new(GRAVEL_ID, 0, 33),
        0,
        128,
    );

    let extra_gravel = random.next_int(16) == 0;
    if extra_gravel {
        decorate_depth_span(
            neighborhood,
            random,
            80,
            OreSpec::new(GRAVEL_ID, 0, 33),
            0,
            50,
        );
    }

    decorate_depth_span(
        neighborhood,
        random,
        10,
        OreSpec::new(STONE_ID, STONE_DIORITE, 33),
        0,
        80,
    );
    decorate_depth_span(
        neighborhood,
        random,
        10,
        OreSpec::new(STONE_ID, STONE_GRANITE, 33),
        0,
        80,
    );
    decorate_depth_span(
        neighborhood,
        random,
        10,
        OreSpec::new(STONE_ID, STONE_ANDESITE, 33),
        0,
        80,
    );
    decorate_depth_span(
        neighborhood,
        random,
        20,
        OreSpec::new(COAL_ORE_ID, 0, 17),
        0,
        128,
    );
    decorate_depth_span(
        neighborhood,
        random,
        20,
        OreSpec::new(IRON_ORE_ID, 0, 9),
        0,
        64,
    );
    decorate_depth_span(
        neighborhood,
        random,
        2,
        OreSpec::new(GOLD_ORE_ID, 0, 9),
        0,
        32,
    );
    decorate_depth_span(
        neighborhood,
        random,
        8,
        OreSpec::new(REDSTONE_ORE_ID, 0, 8),
        0,
        16,
    );
    decorate_depth_span(
        neighborhood,
        random,
        1,
        OreSpec::new(DIAMOND_ORE_ID, 0, 8),
        0,
        16,
    );
    decorate_depth_average(
        neighborhood,
        random,
        1,
        OreSpec::new(LAPIS_ORE_ID, 0, 7),
        16,
        16,
    );

    extra_gravel
}

fn decorate_depth_span(
    neighborhood: &mut PopulationNeighborhood,
    random: &mut MtRandom,
    count: usize,
    ore: OreSpec,
    y0: i32,
    y1: i32,
) {
    let origin_x = neighborhood.center().x().wrapping_mul(16);
    let origin_z = neighborhood.center().z().wrapping_mul(16);
    for _ in 0..count {
        let z = origin_z.wrapping_add(random.next_int(16) as i32);
        let y = random.next_int((y1 - y0) as u32) as i32 + y0;
        let x = origin_x.wrapping_add(random.next_int(16) as i32);
        place_ore(neighborhood, random, BlockPos::new(x, y, z), ore);
    }
}

fn decorate_depth_average(
    neighborhood: &mut PopulationNeighborhood,
    random: &mut MtRandom,
    count: usize,
    ore: OreSpec,
    y_mid: i32,
    y_span: i32,
) {
    let origin_x = neighborhood.center().x().wrapping_mul(16);
    let origin_z = neighborhood.center().z().wrapping_mul(16);
    for _ in 0..count {
        let z = origin_z.wrapping_add(random.next_int(16) as i32);
        let y0 = random.next_int(y_span as u32) as i32;
        let y1 = random.next_int(y_span as u32) as i32 + y_mid - y_span;
        let x = origin_x.wrapping_add(random.next_int(16) as i32);
        place_ore(neighborhood, random, BlockPos::new(x, y0 + y1, z), ore);
    }
}

pub(crate) fn place_ore_feature(
    neighborhood: &mut PopulationNeighborhood,
    random: &mut MtRandom,
    position: (i32, i32, i32),
    block_id: u16,
    data: u8,
    size: usize,
) {
    place_ore(
        neighborhood,
        random,
        BlockPos::new(position.0, position.1, position.2),
        OreSpec::new(block_id, data, size),
    );
}

fn place_ore(
    neighborhood: &mut PopulationNeighborhood,
    random: &mut MtRandom,
    origin: BlockPos,
    ore: OreSpec,
) {
    let direction = random.next_float() * PI;
    let scale = ore.size as f32 / 8.0;
    let rot_x = direction.sin() * scale;
    let rot_z = direction.cos() * scale;

    let v0 = Vec3f::new(
        origin.x as f32 + 8.0 + rot_x,
        origin.y as f32 + random.next_int(3) as f32 - 2.0,
        origin.z as f32 + 8.0 + rot_z,
    );
    let v1 = Vec3f::new(
        origin.x as f32 + 8.0 - rot_x,
        origin.y as f32 + random.next_int(3) as f32 - 2.0,
        origin.z as f32 + 8.0 - rot_z,
    );

    for step in 0..ore.size {
        let d = step as f32;
        let fraction = d / ore.size as f32;
        let center = Vec3f::new(
            v0.x + (v1.x - v0.x) * fraction,
            v0.y + (v1.y - v0.y) * fraction,
            v0.z + (v1.z - v0.z) * fraction,
        );

        let size = random.next_float() * ore.size as f32 / 16.0;
        let radius = ((d * PI / ore.size as f32).sin() + 1.0) * size + 1.0;
        let half = radius / 2.0;
        let inverse_half = 1.0 / half;

        let min = BlockPos::from_floats(center.x - half, center.y - half, center.z - half);
        let max = BlockPos::from_floats(center.x + half, center.y + half, center.z + half);

        for x in min.x..=max.x {
            let xd = (x as f32 + 0.5 - center.x) * inverse_half;
            if xd * xd >= 1.0 {
                continue;
            }
            for y in min.y..=max.y {
                let yd = (y as f32 + 0.5 - center.y) * inverse_half;
                if xd * xd + yd * yd >= 1.0 {
                    continue;
                }
                for z in min.z..=max.z {
                    let zd = (z as f32 + 0.5 - center.z) * inverse_half;
                    if xd * xd + yd * yd + zd * zd >= 1.0 {
                        continue;
                    }
                    if neighborhood.block_id(x, y, z) == STONE_ID {
                        neighborhood.set_state(x, y, z, state(ore.block_id, ore.data));
                    }
                }
            }
        }
    }
}

fn is_mesa_biome(id: u8) -> bool {
    matches!(id, 37..=39 | 165..=167)
}

#[derive(Debug, Copy, Clone)]
struct OreSpec {
    block_id: u16,
    data: u8,
    size: usize,
}

impl OreSpec {
    const fn new(block_id: u16, data: u8, size: usize) -> Self {
        Self {
            block_id,
            data,
            size,
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct BlockPos {
    x: i32,
    y: i32,
    z: i32,
}

impl BlockPos {
    const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    fn from_floats(x: f32, y: f32, z: f32) -> Self {
        Self {
            x: x.floor() as i32,
            y: y.floor() as i32,
            z: z.floor() as i32,
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct Vec3f {
    x: f32,
    y: f32,
    z: f32,
}

impl Vec3f {
    const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChunkCoord, OverworldCaveCarver};

    fn stone_neighborhood(center: ChunkCoord) -> PopulationNeighborhood {
        PopulationNeighborhood::filled(center, state(STONE_ID, 0), 1)
    }

    fn hash_neighborhood(neighborhood: &PopulationNeighborhood) -> u64 {
        neighborhood
            .state_planes()
            .flat_map(|states| states.iter())
            .fold(0xcbf2_9ce4_8422_2325_u64, |mut hash, state| {
                for byte in state.to_le_bytes() {
                    hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
                }
                hash
            })
    }

    fn changed_neighbor_states(neighborhood: &PopulationNeighborhood) -> usize {
        (-1..=1)
            .flat_map(|offset_z| (-1..=1).map(move |offset_x| (offset_x, offset_z)))
            .filter(|(offset_x, offset_z)| *offset_x != 0 || *offset_z != 0)
            .flat_map(|(offset_x, offset_z)| {
                neighborhood
                    .chunk_states(offset_x, offset_z)
                    .expect("population neighbor exists")
                    .iter()
            })
            .filter(|state_id| **state_id != state(STONE_ID, 0))
            .count()
    }

    fn block_count(neighborhood: &PopulationNeighborhood, id: u16) -> usize {
        neighborhood
            .state_planes()
            .flat_map(|states| states.iter())
            .filter(|state_id| (**state_id >> 4) == id)
            .count()
    }

    #[test]
    fn independent_synthetic_ore_neighborhood_fixtures_match() {
        let fixtures = [
            (0, -78, -128, false, false, 0x2602_8c22_9be6_f10e_u64, 4005),
            (0, -58, -128, true, false, 0xad18_2c2d_8133_324c_u64, 3671),
            (-1, 36, -96, false, false, 0x6b29_13de_d4c1_9725_u64, 3643),
            (
                i32::MIN,
                7,
                -9,
                false,
                false,
                0x4683_0fa2_982e_ab1e_u64,
                3852,
            ),
            (0, -1, -16, false, true, 0x78ad_9228_f695_3762_u64, 8072),
        ];

        for (seed, x, z, mesa, expected_extra, expected_hash, expected_neighbor_writes) in fixtures
        {
            let center = ChunkCoord::new(x, z);
            let mut neighborhood = stone_neighborhood(center);
            let mut random = population_random(u32::from_ne_bytes(seed.to_ne_bytes()), center);
            let extra = decorate_common_ores(&mut neighborhood, &mut random);
            if mesa {
                decorate_depth_span(
                    &mut neighborhood,
                    &mut random,
                    20,
                    OreSpec::new(GOLD_ORE_ID, 0, 9),
                    32,
                    80,
                );
            }

            assert_eq!(extra, expected_extra, "seed={seed} center={x}:{z}");
            assert_eq!(
                hash_neighborhood(&neighborhood),
                expected_hash,
                "seed={seed} center={x}:{z}",
            );
            assert_eq!(
                changed_neighbor_states(&neighborhood),
                expected_neighbor_writes,
                "cross-chunk writes seed={seed} center={x}:{z}",
            );
        }
    }

    #[test]
    fn independent_extra_gravel_and_mesa_gold_paths_match() {
        let extra_center = ChunkCoord::new(-1, -16);
        let mut extra = stone_neighborhood(extra_center);
        let mut extra_random = population_random(0, extra_center);
        assert!(decorate_common_ores(&mut extra, &mut extra_random));
        assert_eq!(block_count(&extra, GRAVEL_ID), 7333);

        let mesa_center = ChunkCoord::new(-58, -128);
        let mut mesa = stone_neighborhood(mesa_center);
        let mut mesa_random = population_random(0, mesa_center);
        assert!(!decorate_common_ores(&mut mesa, &mut mesa_random));
        decorate_depth_span(
            &mut mesa,
            &mut mesa_random,
            20,
            OreSpec::new(GOLD_ORE_ID, 0, 9),
            32,
            80,
        );
        assert_eq!(block_count(&mesa, GOLD_ORE_ID), 137);
    }

    #[test]
    fn population_seed_is_stable_across_signed_seed_and_coordinate_boundaries() {
        for seed in [0_u32, 1, u32::MAX, 0x8000_0000, 0x1234_5678] {
            for center in [
                ChunkCoord::new(0, 0),
                ChunkCoord::new(-1, -1),
                ChunkCoord::new(7, -9),
            ] {
                let mut first = population_random(seed, center);
                let mut second = population_random(seed, center);
                for _ in 0..16 {
                    assert_eq!(first.next_u32(), second.next_u32());
                }
            }
        }
    }

    #[test]
    fn population_neighborhood_resolves_negative_world_coordinates() {
        let mut decorated = PopulationNeighborhood::from_carved(0, ChunkCoord::new(-1, -1));
        OverworldOreDecorator::new(0).decorate(&mut decorated);
        let origin_x = -16;
        let origin_z = -16;
        assert!(decorated.state(origin_x - 1, 32, origin_z - 1).is_some());
        assert!(decorated.state(origin_x + 31, 32, origin_z + 31).is_some());
        assert!(decorated.state(origin_x - 17, 32, origin_z).is_none());
        assert!(decorated.state(origin_x, -1, origin_z).is_none());
        assert!(decorated.state(origin_x, 128, origin_z).is_none());
    }

    #[test]
    fn ore_decoration_writes_expected_fixed_target_states() {
        let mut decorated = PopulationNeighborhood::from_carved(0, ChunkCoord::new(-78, -128));
        OverworldOreDecorator::new(0).decorate(&mut decorated);
        let mut saw_ore = false;
        let mut saw_variant = false;
        for states in decorated.state_planes() {
            for state in states {
                let id = state >> 4;
                if matches!(
                    id,
                    GOLD_ORE_ID
                        | IRON_ORE_ID
                        | COAL_ORE_ID
                        | LAPIS_ORE_ID
                        | DIAMOND_ORE_ID
                        | REDSTONE_ORE_ID
                ) {
                    saw_ore = true;
                }
                if id == STONE_ID && matches!((state & 0x0f) as u8, 1 | 3 | 5) {
                    saw_variant = true;
                }
            }
        }
        assert!(saw_ore);
        assert!(saw_variant);
    }

    #[test]
    fn real_ore_decoration_preserves_cross_chunk_population_writes() {
        let center = ChunkCoord::new(-78, -128);
        let decorator = OverworldOreDecorator::new(0);
        let mut decorated = PopulationNeighborhood::from_carved(0, center);
        decorator.decorate(&mut decorated);
        let carver = OverworldCaveCarver::new(0);

        let mut neighbor_changed = false;
        for offset_z in -1..=1 {
            for offset_x in -1..=1 {
                if offset_x == 0 && offset_z == 0 {
                    continue;
                }

                let before = carver.generate(ChunkCoord::new(
                    center.x().wrapping_add(offset_x),
                    center.z().wrapping_add(offset_z),
                ));
                let after = decorated
                    .chunk_states(offset_x, offset_z)
                    .expect("population neighbor exists");
                if before.states() != after {
                    neighbor_changed = true;
                }
            }
        }

        assert!(neighbor_changed);
    }

    #[test]
    fn ore_decoration_is_deterministic() {
        for (seed, center) in [
            (0, ChunkCoord::new(-78, -128)),
            (-1, ChunkCoord::new(36, -96)),
            (i32::MIN, ChunkCoord::new(7, -9)),
        ] {
            let decorator = OverworldOreDecorator::new(seed);
            let mut first = PopulationNeighborhood::from_carved(seed, center);
            let mut second = PopulationNeighborhood::from_carved(seed, center);
            decorator.decorate(&mut first);
            decorator.decorate(&mut second);
            assert_eq!(first, second, "seed={seed} center={center:?}");
        }
    }
}
