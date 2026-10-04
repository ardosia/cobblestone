use crate::terrain_shape::noise::{MtRandom, PerlinSimplexNoise};
use crate::{CHUNK_BLOCK_COUNT, CHUNK_COLUMN_COUNT, ChunkCoord, OverworldTerrainShape};

const WORLD_HEIGHT: usize = 128;
const SEA_LEVEL: i32 = 63;

const AIR: u16 = state(0, 0);
const STONE: u16 = state(1, 0);
const GRASS: u16 = state(2, 0);
const DIRT: u16 = state(3, 0);
const COARSE_DIRT: u16 = state(3, 1);
const BEDROCK: u16 = state(7, 0);
const STILL_WATER: u16 = state(9, 0);
const SAND: u16 = state(12, 0);
const RED_SAND: u16 = state(12, 1);
const GRAVEL: u16 = state(13, 0);
const SANDSTONE: u16 = state(24, 0);
const ICE: u16 = state(79, 0);
const SNOW: u16 = state(80, 0);
const MYCELIUM: u16 = state(110, 0);
const WATER_LILY: u16 = state(111, 0);
const STAINED_CLAY_ID: u16 = 159;
const HARDENED_CLAY: u16 = state(172, 0);
const PODZOL: u16 = state(243, 0);

const CLAY_ORANGE: u8 = 1;
const CLAY_YELLOW: u8 = 4;
const CLAY_SILVER: u8 = 8;
const CLAY_BROWN: u8 = 12;
const CLAY_RED: u8 = 14;
const CLAY_WHITE: u8 = 0;
const HARD_CLAY_BAND: u8 = 16;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SurfacedChunk {
    states: Vec<u16>,
    biome_ids: Vec<u8>,
}

impl SurfacedChunk {
    pub fn states(&self) -> &[u16] {
        &self.states
    }

    pub fn biome_ids(&self) -> &[u8] {
        &self.biome_ids
    }
}

/// Exact fixed-target RandomLevelSource::buildSurfaces stage.
///
/// Density shape, biome selection, caves and population remain separate stages. This consumes the
/// immutable base shape and replaces only the surface/bedrock materials owned by the target pass.
pub struct OverworldSurfaceBuilder {
    terrain: OverworldTerrainShape,
    biome_info_noise: PerlinSimplexNoise,
    mesa: MesaSurface,
}

impl OverworldSurfaceBuilder {
    pub fn new(seed: i32) -> Self {
        let mut biome_info_random = MtRandom::new(2345);
        Self {
            terrain: OverworldTerrainShape::new(seed),
            biome_info_noise: PerlinSimplexNoise::new(&mut biome_info_random, 1),
            mesa: MesaSurface::new(seed),
        }
    }

    pub fn generate(&self, position: ChunkCoord) -> SurfacedChunk {
        let mut states = self.terrain.generate(position).into_states();
        let block_x = position.x().wrapping_mul(16);
        let block_z = position.z().wrapping_mul(16);
        let biome_ids = self
            .terrain
            .biome_source()
            .biome_ids(block_x, block_z, 16, 16);
        let depths = self.terrain.surface_depths(position);

        debug_assert_eq!(states.len(), CHUNK_BLOCK_COUNT);
        debug_assert_eq!(biome_ids.len(), CHUNK_COLUMN_COUNT);
        debug_assert_eq!(depths.len(), CHUNK_COLUMN_COUNT);

        let chunk_seed = position
            .x()
            .wrapping_mul(341_872_712)
            .wrapping_add(position.z().wrapping_mul(132_899_541));
        let mut random = MtRandom::new(u32::from_ne_bytes(chunk_seed.to_ne_bytes()));

        // Target ordering is X outer, Z inner. RNG consumption makes this observable.
        for x in 0..16_usize {
            for z in 0..16_usize {
                let column = x + z * 16;
                let biome = biome_ids[column];
                let depth = depths[column];
                let world_x = block_x.wrapping_add(x as i32);
                let world_z = block_z.wrapping_add(z as i32);

                match biome {
                    37..=39 | 165..=167 => self.mesa.build_column(
                        &mut random,
                        &mut states,
                        x,
                        z,
                        world_x,
                        world_z,
                        biome,
                        depth,
                    ),
                    3 | 20 | 34 | 131 | 162 => {
                        let (top, filler) = extreme_hills_materials(biome, depth);
                        build_default_column(
                            &mut random,
                            &mut states,
                            x,
                            z,
                            biome,
                            depth,
                            top,
                            filler,
                        );
                    }
                    32 | 33 | 160 | 161 => {
                        let top = taiga_top(depth);
                        build_default_column(
                            &mut random,
                            &mut states,
                            x,
                            z,
                            biome,
                            depth,
                            top,
                            DIRT,
                        );
                    }
                    6 | 134 => {
                        swamp_prepass(&self.biome_info_noise, &mut states, x, z, world_x, world_z);
                        let (top, filler) = base_materials(biome);
                        build_default_column(
                            &mut random,
                            &mut states,
                            x,
                            z,
                            biome,
                            depth,
                            top,
                            filler,
                        );
                    }
                    163 | 164 => {
                        let (top, filler) = mutated_savanna_materials(depth);
                        build_default_column(
                            &mut random,
                            &mut states,
                            x,
                            z,
                            biome,
                            depth,
                            top,
                            filler,
                        );
                    }
                    _ => {
                        let (top, filler) = base_materials(biome);
                        build_default_column(
                            &mut random,
                            &mut states,
                            x,
                            z,
                            biome,
                            depth,
                            top,
                            filler,
                        );
                    }
                }
            }
        }

        SurfacedChunk { states, biome_ids }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_default_column(
    random: &mut MtRandom,
    states: &mut [u16],
    x: usize,
    z: usize,
    biome: u8,
    depth: f32,
    default_top: u16,
    default_filler: u16,
) {
    let mut top = default_top;
    let mut filler = default_filler;
    let mut run = -1_i32;
    let run_depth = (depth / 3.0 + 3.0 + random.next_float() * 0.25) as i32;

    place_bedrock(random, states, x, z);

    for y in (1..WORLD_HEIGHT).rev() {
        let index = block_index(x, y, z);
        let old = states[index];

        if old == BEDROCK {
            break;
        }
        if old == AIR {
            run = -1;
            continue;
        }
        if old != STONE {
            continue;
        }

        if run == -1 {
            if run_depth <= 0 {
                top = AIR;
                filler = STONE;
            } else if y as i32 >= SEA_LEVEL - 4 && y as i32 <= SEA_LEVEL + 1 {
                top = default_top;
                filler = default_filler;
            }

            if (y as i32) < SEA_LEVEL && top == AIR {
                top = if is_freezing(biome) { ICE } else { STILL_WATER };
            }

            run = run_depth;
            if y as i32 >= SEA_LEVEL - 1 {
                states[index] = top;
            } else if (y as i32) < SEA_LEVEL - 7 - run_depth {
                top = AIR;
                filler = STONE;
                states[index] = GRAVEL;
            } else {
                states[index] = filler;
            }
        } else if run > 0 {
            run -= 1;
            states[index] = filler;
            if run == 0 && filler == SAND {
                run = random.next_int(4) as i32 + (y as i32 - 63).max(0);
                filler = SANDSTONE;
            }
        }
    }
}

fn place_bedrock(random: &mut MtRandom, states: &mut [u16], x: usize, z: usize) {
    let highest = random.next_int(4) as usize + 1;
    for y in 0..=highest {
        states[block_index(x, y, z)] = BEDROCK;
    }
}

fn base_materials(biome: u8) -> (u16, u16) {
    match parent_biome(biome) {
        2 | 17 => (SAND, SAND),
        14 | 15 => (MYCELIUM, DIRT),
        16 | 26 => (SAND, SAND),
        25 => (STONE, STONE),
        12 if biome == 140 => (SNOW, DIRT),
        _ => (GRASS, DIRT),
    }
}

fn parent_biome(biome: u8) -> u8 {
    match biome {
        129 => 1,
        130 => 2,
        131 => 3,
        132 => 4,
        133 => 5,
        134 => 6,
        140 => 12,
        149 => 21,
        151 => 23,
        155 => 27,
        156 => 28,
        157 => 29,
        158 => 30,
        160 => 32,
        161 => 33,
        162 => 34,
        163 => 35,
        164 => 36,
        165 => 37,
        166 => 38,
        167 => 39,
        other => other,
    }
}

fn is_freezing(biome: u8) -> bool {
    matches!(parent_biome(biome), 10 | 11 | 12 | 13 | 26 | 30 | 31)
}

fn extreme_hills_materials(biome: u8, depth: f32) -> (u16, u16) {
    let kind = match biome {
        20 | 34 => 1,
        131 | 162 => 2,
        _ => 0,
    };

    if (depth < -0.1 || depth > 0.2) && kind == 2 {
        (GRAVEL, GRAVEL)
    } else if depth > 1.0 && kind != 1 {
        (STONE, STONE)
    } else {
        (GRASS, DIRT)
    }
}

fn taiga_top(depth: f32) -> u16 {
    if depth > 1.75 {
        COARSE_DIRT
    } else if depth > -0.95 {
        PODZOL
    } else {
        GRASS
    }
}

fn mutated_savanna_materials(depth: f32) -> (u16, u16) {
    if depth > 1.75 {
        (STONE, STONE)
    } else if depth > -0.5 {
        (COARSE_DIRT, DIRT)
    } else {
        (GRASS, DIRT)
    }
}

fn swamp_prepass(
    biome_info_noise: &PerlinSimplexNoise,
    states: &mut [u16],
    x: usize,
    z: usize,
    world_x: i32,
    world_z: i32,
) {
    let ground = biome_info_noise.value(world_x as f32 * 0.25, world_z as f32 * 0.25);
    if ground <= 0.0 {
        return;
    }

    for y in (0..WORLD_HEIGHT).rev() {
        let index = block_index(x, y, z);
        if states[index] == AIR {
            if y == 62 && states[index] != STILL_WATER {
                states[index] = STILL_WATER;
                if ground < 0.12 {
                    states[block_index(x, y + 1, z)] = WATER_LILY;
                }
            }
            break;
        }
    }
}

struct MesaSurface {
    bands: [u8; 64],
    band_offset_noise: PerlinSimplexNoise,
    pillar_noise: PerlinSimplexNoise,
    pillar_roof_noise: PerlinSimplexNoise,
}

impl MesaSurface {
    fn new(seed: i32) -> Self {
        let raw_seed = u32::from_ne_bytes(seed.to_ne_bytes());

        let mut band_random = MtRandom::new(raw_seed);
        let band_offset_noise = PerlinSimplexNoise::new(&mut band_random, 1);
        let bands = generate_mesa_bands(&mut band_random);

        let mut pillar_random = MtRandom::new(raw_seed);
        let pillar_noise = PerlinSimplexNoise::new(&mut pillar_random, 4);
        let pillar_roof_noise = PerlinSimplexNoise::new(&mut pillar_random, 1);

        Self {
            bands,
            band_offset_noise,
            pillar_noise,
            pillar_roof_noise,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build_column(
        &self,
        random: &mut MtRandom,
        states: &mut [u16],
        x: usize,
        z: usize,
        world_x: i32,
        world_z: i32,
        biome: u8,
        depth: f32,
    ) {
        place_bedrock(random, states, x, z);

        let bryce = biome == 165;
        let forest = matches!(biome, 38 | 166);
        let mut min_pillar = 0.0_f32;

        if bryce {
            let pillar = self
                .pillar_noise
                .value(world_x as f32 * 0.25, world_z as f32 * 0.25);
            let buffer = depth.abs().min(pillar);
            if buffer > 0.0 {
                let roof = self
                    .pillar_roof_noise
                    .value(world_x as f32 / 512.0, world_z as f32 / 512.0)
                    .abs();
                min_pillar = buffer * buffer * 2.5;
                let cap = (roof * 50.0).ceil() + 14.0;
                min_pillar = min_pillar.min(cap) + 64.0;
            }
        }

        let run_depth = (depth / 3.0 + 3.0 + random.next_float() * 0.25) as i32;
        let clay_surface = (depth / 3.0 * std::f32::consts::PI).cos() > 0.0;
        let mut run = -1_i32;
        let mut sand_run = false;
        let mut clay_depth = 0_i32;
        let mut top = stained_clay(CLAY_WHITE);
        let mut filler = HARDENED_CLAY;

        for y in (1..WORLD_HEIGHT).rev() {
            let index = block_index(x, y, z);
            let mut old = states[index];

            if old == BEDROCK {
                break;
            }

            if (old == AIR || block_id(old) == 8 || old == STILL_WATER) && (y as f32) < min_pillar {
                states[index] = STONE;
                old = STONE;
            }

            if clay_depth >= 15 {
                continue;
            }

            if old == AIR {
                run = -1;
                continue;
            }
            if old != STONE {
                continue;
            }

            if run == -1 {
                sand_run = false;
                if run_depth <= 0 {
                    top = AIR;
                    filler = STONE;
                } else if y as i32 >= SEA_LEVEL - 4 && y as i32 <= SEA_LEVEL + 1 {
                    top = stained_clay(CLAY_WHITE);
                    filler = HARDENED_CLAY;
                }

                if (y as i32) < SEA_LEVEL && top == AIR {
                    top = STILL_WATER;
                }

                run = run_depth + (y as i32 - SEA_LEVEL).max(0);

                if y as i32 >= SEA_LEVEL - 1 {
                    if forest && y as i32 > 86 + run_depth * 2 {
                        states[index] = if clay_surface { COARSE_DIRT } else { GRASS };
                    } else if y as i32 > SEA_LEVEL + 3 + run_depth {
                        let data = if y < 64 {
                            CLAY_ORANGE
                        } else if clay_surface {
                            HARD_CLAY_BAND
                        } else {
                            self.band(world_x, y as i32, world_z)
                        };
                        states[index] = mesa_clay(data);
                    } else {
                        states[index] = RED_SAND;
                        sand_run = true;
                    }
                } else {
                    states[index] = filler;
                    if block_id(filler) == STAINED_CLAY_ID {
                        states[index] = stained_clay(CLAY_ORANGE);
                    }
                }
            } else if run > 0 {
                run -= 1;
                if sand_run {
                    states[index] = stained_clay(CLAY_ORANGE);
                } else {
                    states[index] = mesa_clay(self.band(world_x, y as i32, world_z));
                }
            }

            clay_depth += 1;
        }
    }

    fn band(&self, world_x: i32, y: i32, world_z: i32) -> u8 {
        let offset = (self
            .band_offset_noise
            .value(world_x as f32 / 512.0, world_z as f32 / 512.0)
            * 2.0)
            .round_ties_even() as i32;
        self.bands[(y + offset + 64).rem_euclid(64) as usize]
    }
}

fn generate_mesa_bands(random: &mut MtRandom) -> [u8; 64] {
    let mut bands = [HARD_CLAY_BAND; 64];

    let mut i = 0_usize;
    while i < 64 {
        i += random.next_int(5) as usize + 1;
        if i < 64 {
            bands[i] = CLAY_ORANGE;
        }
        i += 1;
    }

    let yellow_count = random.next_int(4) + 2;
    for _ in 0..yellow_count {
        let width = random.next_int(3) as usize + 1;
        let start = random.next_int(64) as usize;
        paint_band(&mut bands, start, width, CLAY_YELLOW);
    }

    let brown_count = random.next_int(4) + 2;
    for _ in 0..brown_count {
        let width = random.next_int(3) as usize + 2;
        let start = random.next_int(64) as usize;
        paint_band(&mut bands, start, width, CLAY_BROWN);
    }

    let red_count = random.next_int(4) + 2;
    for _ in 0..red_count {
        let width = random.next_int(3) as usize + 1;
        let start = random.next_int(64) as usize;
        paint_band(&mut bands, start, width, CLAY_RED);
    }

    let mut start = 0_usize;
    for _ in 0..(random.next_int(3) + 3) {
        start += random.next_int(16) as usize + 4;
        if start >= 64 {
            break;
        }
        bands[start] = CLAY_WHITE;
        if start > 1 && next_boolean(random) {
            bands[start - 1] = CLAY_SILVER;
        }
        if start < 63 && next_boolean(random) {
            bands[start + 1] = CLAY_SILVER;
        }
    }

    bands
}

fn paint_band(bands: &mut [u8; 64], start: usize, width: usize, value: u8) {
    for offset in 0..width {
        if start + offset < 64 {
            bands[start + offset] = value;
        }
    }
}

fn next_boolean(random: &mut MtRandom) -> bool {
    random.next_u32() & 0x0800_0000 != 0
}

const fn state(id: u16, data: u16) -> u16 {
    (id << 4) | data
}

const fn block_id(state: u16) -> u16 {
    state >> 4
}

const fn stained_clay(data: u8) -> u16 {
    state(STAINED_CLAY_ID, data as u16)
}

const fn mesa_clay(data: u8) -> u16 {
    if data < 16 {
        stained_clay(data)
    } else {
        HARDENED_CLAY
    }
}

const fn block_index(x: usize, y: usize, z: usize) -> usize {
    (y << 8) | (z << 4) | x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surfaced_stage_adds_bedrock_and_registered_surface_materials() {
        let chunk = OverworldSurfaceBuilder::new(42).generate(ChunkCoord::new(0, 0));
        assert_eq!(chunk.states().len(), CHUNK_BLOCK_COUNT);
        assert_eq!(chunk.biome_ids().len(), CHUNK_COLUMN_COUNT);
        assert!(chunk.states().contains(&BEDROCK));
        assert!(
            chunk
                .states()
                .iter()
                .all(|state| crate::block_state_id_is_supported(*state))
        );
        for x in 0..16 {
            for z in 0..16 {
                assert_eq!(chunk.states()[block_index(x, 0, z)], BEDROCK);
            }
        }
    }

    #[test]
    fn chunk_surface_is_deterministic_across_coordinate_and_seed_boundaries() {
        for seed in [0, 1, -1, i32::MIN, i32::MAX, 0x1234_5678] {
            for position in [
                ChunkCoord::new(0, 0),
                ChunkCoord::new(-1, -1),
                ChunkCoord::new(7, -9),
            ] {
                let first = OverworldSurfaceBuilder::new(seed).generate(position);
                let second = OverworldSurfaceBuilder::new(seed).generate(position);
                assert_eq!(first, second, "seed={seed} chunk={position:?}");
            }
        }
    }

    #[test]
    fn independent_surfaced_chunk_fixtures_match() {
        // Oracle inputs: #24 parity-locked base terrain + cubiomes MC_1_8 final biome bytes.
        // Surface transformation/noise was reproduced independently in standalone C++ from the
        // target-confirmed MT/Simplex + biome surface rules.
        let fixtures = [
            (0, -78, -128, 1_u8, 0x387e_c5bc_abc6_56d2_u64),
            (0, 75, -128, 2, 0xf9ec_080e_e31b_6a11),
            (0, -58, -128, 37, 0x43b0_5aab_3fa7_6193),
            (0, -37, -128, 165, 0xccff_7b57_1083_3d85),
            (0, 111, -120, 6, 0x5772_b344_ffb8_9115),
            (0, 117, -104, 32, 0xf910_d263_e67e_5d55),
            (0, 122, -7, 131, 0x3cd3_5f81_7500_40e5),
            (0, 2, 78, 163, 0xf471_cce3_8b74_560c),
            (-1, 36, -96, 140, 0x2b97_6234_4889_bd86),
        ];

        for (seed, chunk_x, chunk_z, required_biome, expected) in fixtures {
            let chunk =
                OverworldSurfaceBuilder::new(seed).generate(ChunkCoord::new(chunk_x, chunk_z));
            assert!(
                chunk.biome_ids().contains(&required_biome),
                "fixture lost target biome {required_biome}: seed={seed} chunk={chunk_x}:{chunk_z}",
            );
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
            assert_eq!(
                actual, expected,
                "seed={seed} chunk={chunk_x}:{chunk_z} biome={required_biome}",
            );
        }
    }

    #[test]
    fn dynamic_surface_thresholds_match_fixed_target_semantics() {
        assert_eq!(extreme_hills_materials(131, -0.2), (GRAVEL, GRAVEL));
        assert_eq!(extreme_hills_materials(3, 1.1), (STONE, STONE));
        assert_eq!(extreme_hills_materials(34, 1.1), (GRASS, DIRT));
        assert_eq!(taiga_top(2.0), COARSE_DIRT);
        assert_eq!(taiga_top(0.0), PODZOL);
        assert_eq!(mutated_savanna_materials(2.0), (STONE, STONE));
        assert_eq!(mutated_savanna_materials(0.0), (COARSE_DIRT, DIRT));
    }

    #[test]
    fn mesa_band_generation_is_seed_stable() {
        let mesa = MesaSurface::new(0x1234_5678);
        let second = MesaSurface::new(0x1234_5678);
        assert_eq!(mesa.bands, second.bands);
        assert!(mesa.bands.contains(&CLAY_ORANGE));
        assert!(mesa.bands.contains(&CLAY_WHITE));
    }
}
