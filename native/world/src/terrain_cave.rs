use crate::terrain_shape::noise::MtRandom;
use crate::{CHUNK_BLOCK_COUNT, CHUNK_COLUMN_COUNT, ChunkCoord, OverworldSurfaceBuilder};

const WORLD_HEIGHT: i32 = 128;
const SOURCE_RADIUS: i32 = 8;
const PI: f32 = std::f32::consts::PI;

const AIR_ID: u16 = 0;
const STONE_ID: u16 = 1;
const GRASS_ID: u16 = 2;
const DIRT_ID: u16 = 3;
const FLOWING_WATER_ID: u16 = 8;
const STILL_WATER_ID: u16 = 9;
const STILL_LAVA_ID: u16 = 11;
const SAND_ID: u16 = 12;
const SANDSTONE_ID: u16 = 24;
const MYCELIUM_ID: u16 = 110;
const STAINED_CLAY_ID: u16 = 159;
const HARDENED_CLAY_ID: u16 = 172;
const RED_SANDSTONE_ID: u16 = 179;
const PODZOL_ID: u16 = 243;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CarvedChunk {
    states: Vec<u16>,
    biome_ids: Vec<u8>,
}

impl CarvedChunk {
    pub fn states(&self) -> &[u16] {
        &self.states
    }

    pub fn biome_ids(&self) -> &[u8] {
        &self.biome_ids
    }

    pub(crate) fn into_parts(self) -> (Vec<u16>, Vec<u8>) {
        (self.states, self.biome_ids)
    }
}

/// Exact fixed-target LargeCaveFeature pass used by Infinite Overworld generation.
///
/// The target scans source chunks in an 8-chunk radius around the target chunk. Each source chunk
/// independently seeds its cave systems, so tunnels can enter this target from neighboring chunks.
/// Block IDs are replaced without clearing their legacy metadata nibble, matching the target's
/// separate block-ID/data buffers.
pub struct OverworldCaveCarver {
    seed: u32,
    surface: OverworldSurfaceBuilder,
}

impl OverworldCaveCarver {
    pub fn new(seed: i32) -> Self {
        Self {
            seed: u32::from_ne_bytes(seed.to_ne_bytes()),
            surface: OverworldSurfaceBuilder::new(seed),
        }
    }

    pub fn generate(&self, position: ChunkCoord) -> CarvedChunk {
        let (mut states, biome_ids) = self.surface.generate(position).into_parts();
        debug_assert_eq!(states.len(), CHUNK_BLOCK_COUNT);
        debug_assert_eq!(biome_ids.len(), CHUNK_COLUMN_COUNT);

        let mut random = MtRandom::new(self.seed);
        let x_scale = odd_scale(random.next_positive_int());
        let z_scale = odd_scale(random.next_positive_int());

        let target_x = position.x();
        let target_z = position.z();
        for source_x in target_x.wrapping_sub(SOURCE_RADIUS)..=target_x.wrapping_add(SOURCE_RADIUS)
        {
            for source_z in
                target_z.wrapping_sub(SOURCE_RADIUS)..=target_z.wrapping_add(SOURCE_RADIUS)
            {
                let mixed = source_x
                    .wrapping_mul(x_scale)
                    .wrapping_add(source_z.wrapping_mul(z_scale));
                random.reseed(u32::from_ne_bytes(mixed.to_ne_bytes()) ^ self.seed);
                self.add_feature(&mut states, position, &mut random, source_x, source_z);
            }
        }

        CarvedChunk { states, biome_ids }
    }

    fn add_feature(
        &self,
        states: &mut [u16],
        target: ChunkCoord,
        random: &mut MtRandom,
        source_x: i32,
        source_z: i32,
    ) {
        let rand1 = random.next_int(40) + 1;
        let rand2 = random.next_int(rand1) + 1;
        let mut caves = random.next_int(rand2);
        if random.next_int(15) != 0 {
            caves = 0;
        }

        for _ in 0..caves {
            let cave_z = source_z
                .wrapping_mul(16)
                .wrapping_add(random.next_int(16) as i32);
            let y_param = random.next_int(120) + 8;
            let cave_y = random.next_int(y_param) as i32;
            let cave_x = source_x
                .wrapping_mul(16)
                .wrapping_add(random.next_int(16) as i32);
            let cave = Vec3f::new(cave_x as f32, cave_y as f32, cave_z as f32);

            let mut tunnels = 1;
            if random.next_int(4) == 0 {
                self.add_room(states, target, random, cave);
                tunnels += random.next_int(4);
            }

            for _ in 0..tunnels {
                let y_rot = random.next_float() * PI * 2.0;
                let x_rot = ((random.next_float() - 0.5) * 2.0) / 8.0;
                let thickness = random.next_float() * 2.0 + random.next_float();
                self.add_tunnel(
                    states, target, random, cave, thickness, y_rot, x_rot, 0, 0, 1.0,
                );
            }
        }
    }

    fn add_room(&self, states: &mut [u16], target: ChunkCoord, random: &mut MtRandom, room: Vec3f) {
        let thickness = 1.0 + random.next_float() * 6.0;
        self.add_tunnel(
            states, target, random, room, thickness, 0.0, 0.0, -1, -1, 0.5,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn add_tunnel(
        &self,
        states: &mut [u16],
        target: ChunkCoord,
        source_random: &mut MtRandom,
        start: Vec3f,
        thickness: f32,
        mut y_rot: f32,
        mut x_rot: f32,
        mut step: i32,
        mut dist: i32,
        y_scale: f32,
    ) {
        let x_mid = target.x().wrapping_mul(16).wrapping_add(8) as f32;
        let z_mid = target.z().wrapping_mul(16).wrapping_add(8) as f32;
        let mut cave = start;
        let mut y_rota = 0.0_f32;
        let mut x_rota = 0.0_f32;
        let mut random = MtRandom::new(source_random.next_positive_int());

        if dist <= 0 {
            let max = SOURCE_RADIUS * 16 - 16;
            dist = max - random.next_int((max / 4) as u32) as i32;
        }

        let single_step = if step == -1 {
            step = dist / 2;
            true
        } else {
            false
        };

        let split_point = random.next_int((dist / 2) as u32) as i32 + dist / 4;
        let steep = random.next_int(6) == 0;

        while step < dist {
            let rad = 1.5 + (step as f32 * PI / dist as f32).sin() * thickness;
            let y_rad = rad * y_scale;

            let horizontal = x_rot.cos();
            cave.x += y_rot.cos() * horizontal;
            cave.y += x_rot.sin();
            cave.z += y_rot.sin() * horizontal;

            x_rot *= if steep { 0.92 } else { 0.7 };
            x_rot += x_rota * 0.1;
            y_rot += y_rota * 0.1;

            x_rota *= 0.90;
            y_rota *= 0.75;
            x_rota += random.next_gaussian_float() * random.next_float() * 2.0;
            y_rota += random.next_gaussian_float() * random.next_float() * 4.0;

            if !single_step && step == split_point && thickness > 1.0 {
                let left_thickness = random.next_float() * 0.5 + 0.5;
                self.add_tunnel(
                    states,
                    target,
                    &mut random,
                    cave,
                    left_thickness,
                    y_rot - PI / 2.0,
                    x_rot / 3.0,
                    step,
                    dist,
                    1.0,
                );
                let right_thickness = random.next_float() * 0.5 + 0.5;
                self.add_tunnel(
                    states,
                    target,
                    &mut random,
                    cave,
                    right_thickness,
                    y_rot + PI / 2.0,
                    x_rot / 3.0,
                    step,
                    dist,
                    1.0,
                );
                return;
            }

            if !single_step && random.next_int(4) == 0 {
                step += 1;
                continue;
            }

            let xd = cave.x - x_mid;
            let zd = cave.z - z_mid;
            let remaining = (dist - step) as f32;
            let reach = thickness + 18.0;
            if xd * xd + zd * zd - remaining * remaining > reach * reach {
                return;
            }

            if cave.x < x_mid - 16.0 - rad * 2.0
                || cave.z < z_mid - 16.0 - rad * 2.0
                || cave.x > x_mid + 16.0 + rad * 2.0
                || cave.z > z_mid + 16.0 + rad * 2.0
            {
                step += 1;
                continue;
            }

            let chunk_x = target.x().wrapping_mul(16);
            let chunk_z = target.z().wrapping_mul(16);
            let mut x0 = (cave.x - rad).floor() as i32 - chunk_x - 1;
            let mut x1 = (cave.x + rad).floor() as i32 - chunk_x + 1;
            let mut y0 = (cave.y - y_rad).floor() as i32 - 1;
            let mut y1 = (cave.y + y_rad).floor() as i32 + 1;
            let mut z0 = (cave.z - rad).floor() as i32 - chunk_z - 1;
            let mut z1 = (cave.z + rad).floor() as i32 - chunk_z + 1;

            x0 = x0.max(0);
            x1 = x1.min(16);
            y1 = y1.clamp(1, 120);
            y0 = y0.clamp(1, y1);
            z0 = z0.max(0);
            z1 = z1.min(16);

            if detect_water(states, x0, x1, y0, y1, z0, z1) {
                step += 1;
                continue;
            }

            carve_ellipsoid(states, target, cave, rad, y_rad, x0, x1, y0, y1, z0, z1);

            if single_step {
                break;
            }
            step += 1;
        }
    }
}

fn odd_scale(value: u32) -> i32 {
    let value = value as i32;
    (value / 2).wrapping_mul(2).wrapping_add(1)
}

fn detect_water(states: &mut [u16], x0: i32, x1: i32, y0: i32, y1: i32, z0: i32, z1: i32) -> bool {
    let mut detected = false;
    for x in x0..x1 {
        for z in z0..z1 {
            let mut y = y1 + 1;
            while y >= y0 - 1 {
                if (0..WORLD_HEIGHT).contains(&y) {
                    let index = block_index(x as usize, y as usize, z as usize);
                    let id = block_id(states[index]);
                    if id == FLOWING_WATER_ID || id == STILL_WATER_ID {
                        set_block_id(&mut states[index], FLOWING_WATER_ID);
                        detected = true;
                    }
                }
                if y != y0 - 1 && x != x0 && x != x1 - 1 && z != z0 && z != z1 - 1 {
                    y = y0 - 1;
                } else {
                    y -= 1;
                }
            }
        }
    }
    detected
}

#[allow(clippy::too_many_arguments)]
fn carve_ellipsoid(
    states: &mut [u16],
    target: ChunkCoord,
    cave: Vec3f,
    rad: f32,
    y_rad: f32,
    x0: i32,
    x1: i32,
    y0: i32,
    y1: i32,
    z0: i32,
    z1: i32,
) {
    let chunk_x = target.x().wrapping_mul(16);
    let chunk_z = target.z().wrapping_mul(16);

    for x in x0..x1 {
        let xd = (x as f32 + chunk_x as f32 + 0.5 - cave.x) / rad;
        for z in z0..z1 {
            let zd = (z as f32 + chunk_z as f32 + 0.5 - cave.z) / rad;
            if xd * xd + zd * zd >= 1.0 {
                continue;
            }

            let mut has_grass = false;
            let mut state_y = y1;
            let mut y = y1 - 1;
            while y >= y0 {
                let yd = (y as f32 + 0.5 - cave.y) / y_rad;
                if yd > -0.7 && xd * xd + yd * yd + zd * zd < 1.0 {
                    let index = block_index(x as usize, state_y as usize, z as usize);
                    let above_y = state_y + 1;
                    let above_id = if above_y < WORLD_HEIGHT {
                        block_id(states[block_index(x as usize, above_y as usize, z as usize)])
                    } else {
                        AIR_ID
                    };

                    if block_id(states[index]) == GRASS_ID {
                        has_grass = true;
                    }

                    if is_diggable(block_id(states[index]), above_id) {
                        if y < 10 {
                            set_block_id(&mut states[index], STILL_LAVA_ID);
                        } else {
                            if thin_sand(states, x as usize, state_y, z as usize) {
                                let above_index =
                                    block_index(x as usize, (state_y + 1) as usize, z as usize);
                                let replacement = if block_data(states[above_index]) == 1 {
                                    RED_SANDSTONE_ID
                                } else {
                                    SANDSTONE_ID
                                };
                                set_block_id(&mut states[above_index], replacement);
                            }

                            set_block_id(&mut states[index], AIR_ID);
                            if has_grass && state_y > 0 {
                                let below_index =
                                    block_index(x as usize, (state_y - 1) as usize, z as usize);
                                if block_id(states[below_index]) == DIRT_ID {
                                    set_block_id(&mut states[below_index], GRASS_ID);
                                }
                            }
                        }
                    }
                }
                state_y -= 1;
                y -= 1;
            }
        }
    }
}

fn thin_sand(states: &[u16], x: usize, y: i32, z: usize) -> bool {
    if y >= WORLD_HEIGHT - 3 {
        return false;
    }
    let y1 = (y + 1) as usize;
    let y2 = (y + 2) as usize;
    let y3 = (y + 3) as usize;
    block_id(states[block_index(x, y1, z)]) == SAND_ID
        && block_id(states[block_index(x, y2, z)]) == SAND_ID
        && block_id(states[block_index(x, y3, z)]) == SAND_ID
}

fn is_diggable(block: u16, above: u16) -> bool {
    matches!(
        block,
        STONE_ID
            | DIRT_ID
            | GRASS_ID
            | HARDENED_CLAY_ID
            | STAINED_CLAY_ID
            | SANDSTONE_ID
            | RED_SANDSTONE_ID
            | MYCELIUM_ID
            | PODZOL_ID
    ) || (block == SAND_ID && above != STILL_WATER_ID)
}

const fn block_id(state: u16) -> u16 {
    state >> 4
}

const fn block_data(state: u16) -> u16 {
    state & 0x0f
}

fn set_block_id(state: &mut u16, id: u16) {
    *state = (id << 4) | block_data(*state);
}

const fn block_index(x: usize, y: usize, z: usize) -> usize {
    (y << 8) | (z << 4) | x
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

    #[test]
    fn water_boundary_aborts_and_converts_to_flowing_water() {
        let mut states = vec![STONE_ID << 4; CHUNK_BLOCK_COUNT];
        let water = block_index(0, 20, 0);
        states[water] = STILL_WATER_ID << 4;
        assert!(detect_water(&mut states, 0, 3, 18, 22, 0, 3));
        assert_eq!(block_id(states[water]), FLOWING_WATER_ID);
    }

    #[test]
    fn diggable_domain_matches_fixed_target_rules() {
        for id in [
            STONE_ID,
            DIRT_ID,
            GRASS_ID,
            HARDENED_CLAY_ID,
            STAINED_CLAY_ID,
            SANDSTONE_ID,
            RED_SANDSTONE_ID,
            MYCELIUM_ID,
            PODZOL_ID,
        ] {
            assert!(is_diggable(id, AIR_ID), "id={id}");
        }
        assert!(is_diggable(SAND_ID, AIR_ID));
        assert!(!is_diggable(SAND_ID, STILL_WATER_ID));
        for id in [AIR_ID, FLOWING_WATER_ID, STILL_WATER_ID, STILL_LAVA_ID] {
            assert!(!is_diggable(id, AIR_ID), "id={id}");
        }
    }

    #[test]
    fn block_id_replacement_preserves_metadata_nibble() {
        let mut state = (STAINED_CLAY_ID << 4) | 14;
        set_block_id(&mut state, AIR_ID);
        assert_eq!(state, 14);
        assert_eq!(block_id(state), AIR_ID);
        assert_eq!(block_data(state), 14);
    }

    #[test]
    fn independent_cave_chunk_fixtures_match() {
        // Independent oracle chain: #25 standalone C++ surfaced bytes, then a separate C++
        // LargeCaveFeature reproduction using the target-confirmed MCPE MT/random semantics.
        // Every fixture is cave-bearing and includes writes from at least one neighboring source
        // chunk inside the target's radius-8 source scan.
        let fixtures = [
            (0, -78, -128, 0x3568_bd43_38d4_3642_u64),
            (0, -58, -128, 0x9d83_2b9f_c319_40d3_u64),
            (-1, 36, -96, 0x65bc_4fbf_84d0_1fc6_u64),
        ];

        for (seed, chunk_x, chunk_z, expected) in fixtures {
            let chunk = OverworldCaveCarver::new(seed).generate(ChunkCoord::new(chunk_x, chunk_z));
            let actual =
                chunk
                    .states()
                    .iter()
                    .fold(0xcbf2_9ce4_8422_2325_u64, |mut hash, state| {
                        for byte in state.to_le_bytes() {
                            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
                        }
                        hash
                    });
            assert_eq!(actual, expected, "seed={seed} chunk={chunk_x}:{chunk_z}");
        }
    }

    #[test]
    fn cave_stage_is_deterministic_across_seed_and_coordinate_boundaries() {
        for seed in [0, 1, -1, i32::MIN, i32::MAX, 0x1234_5678] {
            for pos in [
                ChunkCoord::new(0, 0),
                ChunkCoord::new(-1, -1),
                ChunkCoord::new(7, -9),
            ] {
                let first = OverworldCaveCarver::new(seed).generate(pos);
                let second = OverworldCaveCarver::new(seed).generate(pos);
                assert_eq!(first, second, "seed={seed} chunk={pos:?}");
                assert_eq!(first.states().len(), CHUNK_BLOCK_COUNT);
                assert_eq!(first.biome_ids().len(), CHUNK_COLUMN_COUNT);
            }
        }
    }

    #[test]
    fn cave_stage_changes_some_fixed_target_chunks() {
        let carver = OverworldCaveCarver::new(0);
        let surface = OverworldSurfaceBuilder::new(0);
        let mut found = false;
        for x in -4..=4 {
            for z in -4..=4 {
                let pos = ChunkCoord::new(x, z);
                if carver.generate(pos).states() != surface.generate(pos).states() {
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }
        assert!(
            found,
            "sample region unexpectedly contained no cave changes"
        );
    }
}
