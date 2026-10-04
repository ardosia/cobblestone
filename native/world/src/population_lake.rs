use crate::population::{PopulationNeighborhood, population_random, state};
use crate::terrain_shape::noise::MtRandom;

const AIR_ID: u16 = 0;
const STONE_ID: u16 = 1;
#[cfg(test)]
const DIRT_ID: u16 = 3;
const FLOWING_WATER_ID: u16 = 8;
const STILL_WATER_ID: u16 = 9;
const FLOWING_LAVA_ID: u16 = 10;
const STILL_LAVA_ID: u16 = 11;

const DESERT_BIOME: u8 = 2;
const DESERT_HILLS_BIOME: u8 = 17;

const GRID_X: usize = 16;
const GRID_Y: usize = 8;
const GRID_Z: usize = 16;
const GRID_SIZE: usize = GRID_X * GRID_Y * GRID_Z;

pub struct OverworldLakePopulator {
    seed: u32,
}

impl OverworldLakePopulator {
    pub fn new(seed: i32) -> Self {
        Self {
            seed: u32::from_ne_bytes(seed.to_ne_bytes()),
        }
    }

    pub fn populate(&self, neighborhood: &mut PopulationNeighborhood) {
        let _ = self.populate_traced(neighborhood);
    }

    fn populate_traced(&self, neighborhood: &mut PopulationNeighborhood) -> LakeTrace {
        let center = neighborhood.center();
        let biome = neighborhood.center_biome_ids()[15 + 15 * 16];
        let mut random = population_random(self.seed, center);
        let origin_x = center.x().wrapping_mul(16);
        let origin_z = center.z().wrapping_mul(16);

        let mut trace = LakeTrace::default();
        let mut has_lake = false;

        if biome != DESERT_BIOME && biome != DESERT_HILLS_BIOME && random.next_int(4) == 0 {
            trace.water_attempted = true;
            let x = origin_x.wrapping_add(random.next_int(9) as i32 + 3);
            let y = random.next_int(128) as i32;
            let z = origin_z.wrapping_add(random.next_int(9) as i32 + 3);
            trace.water_origin = Some(BlockPos::new(x, y, z));
            trace.water_placed = place_lake(
                neighborhood,
                &mut random,
                BlockPos::new(x, y, z),
                STILL_WATER_ID,
            );
            // Target sets hasLake after the feature call even when LakeFeature aborts.
            has_lake = true;
        }

        if !has_lake && random.next_int(8) == 0 {
            trace.lava_attempted = true;
            let x = origin_x.wrapping_add(random.next_int(9) as i32 + 3);
            let y_param = random.next_int(120) + 8;
            let y = random.next_int(y_param) as i32;
            let z = origin_z.wrapping_add(random.next_int(9) as i32 + 3);
            trace.lava_origin = Some(BlockPos::new(x, y, z));

            if y >= 60 && (y < 64 || random.next_int(10) == 0) {
                trace.lava_gate_passed = true;
                trace.lava_placed = place_lake(
                    neighborhood,
                    &mut random,
                    BlockPos::new(x, y, z),
                    STILL_LAVA_ID,
                );
            }
        }

        trace
    }
}

fn place_lake(
    neighborhood: &mut PopulationNeighborhood,
    random: &mut MtRandom,
    origin: BlockPos,
    liquid: u16,
) -> bool {
    let mut base = BlockPos::new(origin.x.wrapping_sub(8), origin.y, origin.z.wrapping_sub(8));

    while base.y > 0 && neighborhood.block_id(base.x, base.y, base.z) == AIR_ID {
        base.y -= 1;
    }
    base.y -= 4;

    let mut grid = [false; GRID_SIZE];
    let spots = random.next_int(4) + 4;

    for _ in 0..spots {
        let rx = random.next_float() * 6.0 + 3.0;
        let ry = random.next_float() * 4.0 + 2.0;
        let rz = random.next_float() * 6.0 + 3.0;

        let mut px = (((16.0_f32 - rx) - 2.0) + 1.0) + rx / 2.0;
        let mut py = (((8.0_f32 - ry) - 4.0) + 2.0) + ry / 2.0;
        let mut pz = (((16.0_f32 - rz) - 2.0) + 1.0) + rz / 2.0;
        px *= random.next_float();
        py *= random.next_float();
        pz *= random.next_float();

        for x in 1..15 {
            for z in 1..15 {
                for y in 1..7 {
                    let xd = (x as f32 - px) / (rx / 2.0);
                    let yd = (y as f32 - py) / (ry / 2.0);
                    let zd = (z as f32 - pz) / (rz / 2.0);
                    if xd * xd + yd * yd + zd * zd < 1.0 {
                        grid[grid_index(x, y, z)] = true;
                    }
                }
            }
        }
    }

    for x in 0..16 {
        for z in 0..16 {
            for y in 0..8 {
                if !is_boundary(&grid, x, y, z) {
                    continue;
                }
                let id = neighborhood.block_id(
                    base.x.wrapping_add(x as i32),
                    base.y + y as i32,
                    base.z.wrapping_add(z as i32),
                );
                if y >= 4 && is_liquid(id) {
                    return false;
                }
                if y < 4 && !is_solid(id) && id != liquid {
                    return false;
                }
            }
        }
    }

    for x in 0..16 {
        for z in 0..16 {
            for y in 0..8 {
                if grid[grid_index(x, y, z)] {
                    let id = if y >= 4 { AIR_ID } else { liquid };
                    neighborhood.set_state(
                        base.x.wrapping_add(x as i32),
                        base.y + y as i32,
                        base.z.wrapping_add(z as i32),
                        state(id, 0),
                    );
                }
            }
        }
    }

    // The target source contains a dirt-to-grass repair guarded by current skylight > 0.
    // Generated Overworld chunks have default skylight MIN/0 until the later post-process light
    // pass, so this branch cannot change fixed-target Infinite lake output here.

    if liquid == STILL_LAVA_ID {
        for x in 0..16 {
            for z in 0..16 {
                for y in 0..8 {
                    if !is_boundary(&grid, x, y, z) {
                        continue;
                    }
                    let replace = y < 4 || random.next_int(2) != 0;
                    if !replace {
                        continue;
                    }
                    let world_x = base.x.wrapping_add(x as i32);
                    let world_y = base.y + y as i32;
                    let world_z = base.z.wrapping_add(z as i32);
                    let id = neighborhood.block_id(world_x, world_y, world_z);
                    if is_solid(id) {
                        neighborhood.set_state(world_x, world_y, world_z, state(STONE_ID, 0));
                    }
                }
            }
        }
    }

    true
}

fn is_boundary(grid: &[bool; GRID_SIZE], x: usize, y: usize, z: usize) -> bool {
    !grid[grid_index(x, y, z)]
        && ((x < 15 && grid[grid_index(x + 1, y, z)])
            || (x > 0 && grid[grid_index(x - 1, y, z)])
            || (z < 15 && grid[grid_index(x, y, z + 1)])
            || (z > 0 && grid[grid_index(x, y, z - 1)])
            || (y < 7 && grid[grid_index(x, y + 1, z)])
            || (y > 0 && grid[grid_index(x, y - 1, z)]))
}

const fn grid_index(x: usize, y: usize, z: usize) -> usize {
    (x * 16 + z) * 8 + y
}

const fn is_liquid(id: u16) -> bool {
    matches!(
        id,
        FLOWING_WATER_ID | STILL_WATER_ID | FLOWING_LAVA_ID | STILL_LAVA_ID
    )
}

const fn is_solid(id: u16) -> bool {
    id != AIR_ID && !is_liquid(id)
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct BlockPos {
    x: i32,
    y: i32,
    z: i32,
}

impl BlockPos {
    const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }
}

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
struct LakeTrace {
    water_attempted: bool,
    water_placed: bool,
    water_origin: Option<BlockPos>,
    lava_attempted: bool,
    lava_gate_passed: bool,
    lava_placed: bool,
    lava_origin: Option<BlockPos>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ChunkCoord;

    fn filled(center: ChunkCoord, block: u16, biome: u8) -> PopulationNeighborhood {
        PopulationNeighborhood::filled(center, state(block, 0), biome)
    }

    fn changed_neighbor_states(neighborhood: &PopulationNeighborhood, original: u16) -> usize {
        (-1..=1)
            .flat_map(|offset_z| (-1..=1).map(move |offset_x| (offset_x, offset_z)))
            .filter(|(offset_x, offset_z)| *offset_x != 0 || *offset_z != 0)
            .flat_map(|(offset_x, offset_z)| {
                neighborhood
                    .chunk_states(offset_x, offset_z)
                    .expect("population neighbor exists")
                    .iter()
            })
            .filter(|state_id| **state_id != state(original, 0))
            .count()
    }

    #[test]
    fn lake_feature_aborts_when_lower_boundary_is_not_solid() {
        let center = ChunkCoord::new(0, 0);
        let mut neighborhood = filled(center, AIR_ID, 1);
        let mut random = population_random(0, center);
        assert!(!place_lake(
            &mut neighborhood,
            &mut random,
            BlockPos::new(8, 64, 8),
            STILL_WATER_ID,
        ));
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

    fn block_count(neighborhood: &PopulationNeighborhood, id: u16) -> usize {
        neighborhood
            .state_planes()
            .flat_map(|states| states.iter())
            .filter(|value| (**value >> 4) == id)
            .count()
    }

    #[test]
    fn independent_water_lake_fixture_matches() {
        let center = ChunkCoord::new(-63, -64);
        let mut neighborhood = filled(center, STONE_ID, 1);
        let trace = OverworldLakePopulator::new(0).populate_traced(&mut neighborhood);

        assert_eq!(
            trace,
            LakeTrace {
                water_attempted: true,
                water_placed: true,
                water_origin: Some(BlockPos::new(-997, 101, -1018)),
                lava_attempted: false,
                lava_gate_passed: false,
                lava_placed: false,
                lava_origin: None,
            }
        );
        assert_eq!(hash_neighborhood(&neighborhood), 0x5e20_b4e8_46e4_8585);
        assert_eq!(changed_neighbor_states(&neighborhood, STONE_ID), 20);
        assert_eq!(block_count(&neighborhood, STILL_WATER_ID), 207);
        assert_eq!(block_count(&neighborhood, AIR_ID), 4);
    }

    #[test]
    fn independent_desert_and_failed_water_gates_match() {
        let center = ChunkCoord::new(-63, -64);
        let populator = OverworldLakePopulator::new(0);

        let mut desert = filled(center, STONE_ID, DESERT_BIOME);
        let desert_trace = populator.populate_traced(&mut desert);
        assert!(!desert_trace.water_attempted);
        assert!(desert_trace.lava_attempted);
        assert!(!desert_trace.lava_gate_passed);
        assert!(!desert_trace.lava_placed);
        assert_eq!(
            desert_trace.lava_origin,
            Some(BlockPos::new(-997, 16, -1015))
        );
        assert_eq!(hash_neighborhood(&desert), 0xcea4_35ca_df56_2325);
        assert_eq!(changed_neighbor_states(&desert, STONE_ID), 0);

        let mut air = filled(center, AIR_ID, 1);
        let failed_trace = populator.populate_traced(&mut air);
        assert!(failed_trace.water_attempted);
        assert!(!failed_trace.water_placed);
        assert!(!failed_trace.lava_attempted);
        assert_eq!(hash_neighborhood(&air), 0x47e2_fabf_b256_2325);
    }

    #[test]
    fn independent_lava_lake_fixture_matches_boundary_stone_conversion() {
        let center = ChunkCoord::new(11, -57);
        let mut neighborhood = filled(center, DIRT_ID, 1);
        let trace = OverworldLakePopulator::new(0).populate_traced(&mut neighborhood);

        assert!(!trace.water_attempted);
        assert!(trace.lava_attempted);
        assert!(trace.lava_gate_passed);
        assert!(trace.lava_placed);
        assert_eq!(trace.lava_origin, Some(BlockPos::new(184, 60, -907)));
        assert_eq!(hash_neighborhood(&neighborhood), 0xaa58_0d1c_9aa9_5d65);
        assert_eq!(changed_neighbor_states(&neighborhood, DIRT_ID), 164);
        assert_eq!(block_count(&neighborhood, STILL_LAVA_ID), 160);
        assert_eq!(block_count(&neighborhood, STONE_ID), 182);
        assert_eq!(block_count(&neighborhood, AIR_ID), 40);
    }

    #[test]
    fn independent_signed_seed_water_fixture_matches() {
        let center = ChunkCoord::new(-63, -64);
        let mut neighborhood = filled(center, STONE_ID, 1);
        let trace = OverworldLakePopulator::new(-1).populate_traced(&mut neighborhood);

        assert!(trace.water_attempted);
        assert!(trace.water_placed);
        assert_eq!(trace.water_origin, Some(BlockPos::new(-1004, 115, -1018)));
        assert!(!trace.lava_attempted);
        assert_eq!(hash_neighborhood(&neighborhood), 0x5afb_2180_df11_d495);
        assert_eq!(changed_neighbor_states(&neighborhood, STONE_ID), 122);
        assert_eq!(block_count(&neighborhood, STILL_WATER_ID), 183);
        assert_eq!(block_count(&neighborhood, AIR_ID), 69);
    }
}
