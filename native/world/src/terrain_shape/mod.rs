mod noise;

use crate::{CHUNK_BLOCK_COUNT, ChunkCoord, OverworldBiomeSource};

use noise::{MtRandom, PerlinNoise, consume_simplex_initialization};

const NOISE_EDGE: usize = 5;
const NOISE_HEIGHT: usize = 17;
const SEA_LEVEL: usize = 63;

const AIR_STATE: u16 = 0;
const STONE_STATE: u16 = 1 << 4;
const STILL_WATER_STATE: u16 = 9 << 4;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ChunkTerrainShape {
    states: Vec<u16>,
}

impl ChunkTerrainShape {
    pub fn states(&self) -> &[u16] {
        &self.states
    }
}

/// Exact fixed-target RandomLevelSource::prepareHeights base-terrain stage.
///
/// This deliberately stops before biome surface replacement. The output contains only air,
/// stone, and still water. Bedrock/top/filler/caves/population belong to later generator stages.
pub struct OverworldTerrainShape {
    biome_source: OverworldBiomeSource,
    min_limit_noise: PerlinNoise,
    max_limit_noise: PerlinNoise,
    main_noise: PerlinNoise,
    depth_noise: PerlinNoise,
    biome_weights: [f32; 25],
}

impl OverworldTerrainShape {
    pub fn new(seed: i32) -> Self {
        let raw_seed = u32::from_ne_bytes(seed.to_ne_bytes());
        let mut random = MtRandom::new(raw_seed);

        let min_limit_noise = PerlinNoise::new(&mut random, 16);
        let max_limit_noise = PerlinNoise::new(&mut random, 16);
        let main_noise = PerlinNoise::new(&mut random, 8);

        // These constructors are part of the target RNG stream even when this stage does not read
        // their values.
        consume_simplex_initialization(&mut random, 4);
        let _scale_noise = PerlinNoise::new(&mut random, 10);
        let depth_noise = PerlinNoise::new(&mut random, 16);
        let _forest_noise = PerlinNoise::new(&mut random, 8);

        let mut biome_weights = [0.0; 25];
        for x in -2_i32..=2 {
            for z in -2_i32..=2 {
                biome_weights[(x + 2 + (z + 2) * 5) as usize] =
                    10.0 / ((x * x + z * z) as f32 + 0.2).sqrt();
            }
        }

        Self {
            biome_source: OverworldBiomeSource::new(seed),
            min_limit_noise,
            max_limit_noise,
            main_noise,
            depth_noise,
            biome_weights,
        }
    }

    pub fn generate(&self, position: ChunkCoord) -> ChunkTerrainShape {
        let noise_x = position.x().wrapping_mul(4);
        let noise_z = position.z().wrapping_mul(4);
        let raw_biomes = self.biome_source.raw_biome_ids(
            noise_x.wrapping_sub(2),
            noise_z.wrapping_sub(2),
            10,
            10,
        );
        let density = self.density_lattice(noise_x, noise_z, &raw_biomes);

        ChunkTerrainShape {
            states: interpolate_shape(&density),
        }
    }

    fn density_lattice(&self, x: i32, z: i32, biomes: &[u8]) -> [f32; 425] {
        debug_assert_eq!(biomes.len(), 100);

        const COORDINATE_SCALE: f32 = 684.412;
        const HEIGHT_SCALE: f32 = 684.412;

        let depth_region = self.depth_noise.region(
            x as f32, 10.0, z as f32, NOISE_EDGE, 1, NOISE_EDGE, 200.0, 1.0, 200.0,
        );
        let main_region = self.main_noise.region(
            x as f32,
            0.0,
            z as f32,
            NOISE_EDGE,
            NOISE_HEIGHT,
            NOISE_EDGE,
            COORDINATE_SCALE / 80.0,
            HEIGHT_SCALE / 160.0,
            COORDINATE_SCALE / 80.0,
        );
        let min_region = self.min_limit_noise.region(
            x as f32,
            0.0,
            z as f32,
            NOISE_EDGE,
            NOISE_HEIGHT,
            NOISE_EDGE,
            COORDINATE_SCALE,
            HEIGHT_SCALE * 1.25,
            COORDINATE_SCALE,
        );
        let max_region = self.max_limit_noise.region(
            x as f32,
            0.0,
            z as f32,
            NOISE_EDGE,
            NOISE_HEIGHT,
            NOISE_EDGE,
            COORDINATE_SCALE,
            HEIGHT_SCALE,
            COORDINATE_SCALE,
        );

        let mut out = [0.0_f32; 425];
        let mut noise_index = 0;
        let mut depth_index = 0;

        for local_x in 0..NOISE_EDGE {
            for local_z in 0..NOISE_EDGE {
                let middle = biome_height(biomes[local_x + 2 + (local_z + 2) * 10]);
                let mut scale_sum = 0.0_f32;
                let mut depth_sum = 0.0_f32;
                let mut weight_sum = 0.0_f32;

                for offset_x in -1_i32..=1 {
                    for offset_z in -1_i32..=1 {
                        let biome_index =
                            (local_x as i32 + offset_x + 2) + (local_z as i32 + offset_z + 2) * 10;
                        let neighbor = biome_height(biomes[biome_index as usize]);
                        let mut weight = self.biome_weights
                            [(offset_x + 2 + (offset_z + 2) * 5) as usize]
                            / (neighbor.depth + 2.0);
                        if neighbor.depth > middle.depth {
                            weight *= 0.5;
                        }

                        scale_sum += neighbor.scale * weight;
                        depth_sum += neighbor.depth * weight;
                        weight_sum += weight;
                    }
                }

                scale_sum /= weight_sum;
                depth_sum /= weight_sum;
                scale_sum = scale_sum * 0.9 + 0.1;
                depth_sum = (depth_sum * 4.0 - 1.0) / 8.0;

                let mut random_depth = depth_region[depth_index] / 8000.0;
                depth_index += 1;
                if random_depth < 0.0 {
                    random_depth = -random_depth * 0.3;
                }
                random_depth = random_depth * 3.0 - 2.0;
                if random_depth < 0.0 {
                    random_depth /= 2.0;
                    if random_depth < -1.0 {
                        random_depth = -1.0;
                    }
                    random_depth /= 1.4;
                    random_depth /= 2.0;
                } else {
                    if random_depth > 1.0 {
                        random_depth = 1.0;
                    }
                    random_depth /= 8.0;
                }

                let mut depth = depth_sum + random_depth * 0.2;
                depth *= 1.0625;
                let center_y = 8.5 + depth * 4.0;

                for local_y in 0..NOISE_HEIGHT {
                    let mut y_offset = (local_y as f32 - center_y) * 12.0 / scale_sum;
                    if y_offset < 0.0 {
                        y_offset *= 4.0;
                    }

                    let min = min_region[noise_index] / 256.0;
                    let max = max_region[noise_index] / 512.0;
                    let blend = (main_region[noise_index] / 10.0 + 1.0) / 2.0;
                    let mut value = clamped_lerp(min, max, blend) - y_offset;

                    if local_y > NOISE_HEIGHT - 4 {
                        let slide = (local_y - (NOISE_HEIGHT - 4)) as f32 / 3.0;
                        value = value * (1.0 - slide) - 10.0 * slide;
                    }

                    out[noise_index] = value;
                    noise_index += 1;
                }
            }
        }

        out
    }
}

#[derive(Copy, Clone)]
struct BiomeHeight {
    depth: f32,
    scale: f32,
}

fn biome_height(id: u8) -> BiomeHeight {
    let pair = match id {
        0 | 10 => (-1.0, 0.1),
        1 | 4 | 5 | 21 | 23 | 27 | 29 | 37 | 38 => (0.1, 0.2),
        2 | 12 | 35 => (0.125, 0.05),
        3 | 34 | 131 | 162 => (1.0, 0.5),
        6 => (-0.2, 0.1),
        7 | 11 => (-0.5, 0.0),
        13 | 17 | 18 | 19 | 22 | 28 | 31 | 33 | 166 | 167 => (0.45, 0.3),
        14 => (0.2, 0.3),
        15 | 16 | 26 => (0.0, 0.025),
        20 => (0.8, 0.4),
        24 => (-1.8, 0.1),
        25 => (0.1, 0.8),
        30 | 32 | 160 => (0.2, 0.2),
        36 | 39 => (1.5, 0.025),
        129 => (0.2, 0.4),
        130 => (0.225, 0.25),
        132 => (0.1, 0.4),
        133 | 149 | 151 | 155 | 157 => (0.2, 0.4),
        134 => (-0.1, 0.3),
        140 => (0.425, 0.45),
        156 | 161 => (0.55, 0.5),
        158 => (0.3, 0.4),
        163 => (0.3625, 1.225),
        164 => (1.05, 1.2125),
        165 => (0.1, 0.2),
        _ => panic!("terrain shape received unsupported fixed-target biome id {id}"),
    };

    BiomeHeight {
        depth: pair.0,
        scale: pair.1,
    }
}

fn interpolate_shape(density: &[f32; 425]) -> Vec<u16> {
    let mut states = vec![AIR_STATE; CHUNK_BLOCK_COUNT];

    for coarse_x in 0..4 {
        for coarse_z in 0..4 {
            for coarse_y in 0..16 {
                let mut near_near = density[density_index(coarse_x, coarse_z, coarse_y)];
                let mut near_far = density[density_index(coarse_x, coarse_z + 1, coarse_y)];
                let mut far_near = density[density_index(coarse_x + 1, coarse_z, coarse_y)];
                let mut far_far = density[density_index(coarse_x + 1, coarse_z + 1, coarse_y)];

                let near_near_step =
                    (density[density_index(coarse_x, coarse_z, coarse_y + 1)] - near_near) / 8.0;
                let near_far_step =
                    (density[density_index(coarse_x, coarse_z + 1, coarse_y + 1)] - near_far) / 8.0;
                let far_near_step =
                    (density[density_index(coarse_x + 1, coarse_z, coarse_y + 1)] - far_near) / 8.0;
                let far_far_step =
                    (density[density_index(coarse_x + 1, coarse_z + 1, coarse_y + 1)] - far_far)
                        / 8.0;

                for local_y in 0..8 {
                    let mut near = near_near;
                    let mut far = near_far;
                    let near_step = (far_near - near_near) / 4.0;
                    let far_step = (far_far - near_far) / 4.0;

                    for local_x in 0..4 {
                        let mut value = near;
                        let z_step = (far - near) / 4.0;
                        value -= z_step;

                        for local_z in 0..4 {
                            value += z_step;
                            let x = coarse_x * 4 + local_x;
                            let z = coarse_z * 4 + local_z;
                            let y = coarse_y * 8 + local_y;
                            states[(y << 8) | (z << 4) | x] = if value > 0.0 {
                                STONE_STATE
                            } else if y < SEA_LEVEL {
                                STILL_WATER_STATE
                            } else {
                                AIR_STATE
                            };
                        }

                        near += near_step;
                        far += far_step;
                    }

                    near_near += near_near_step;
                    near_far += near_far_step;
                    far_near += far_near_step;
                    far_far += far_far_step;
                }
            }
        }
    }

    states
}

const fn density_index(x: usize, z: usize, y: usize) -> usize {
    (x * NOISE_EDGE + z) * NOISE_HEIGHT + y
}

fn clamped_lerp(min: f32, max: f32, value: f32) -> f32 {
    if value < 0.0 {
        min
    } else if value > 1.0 {
        max
    } else {
        min + value * (max - min)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block_id_hash(states: &[u16]) -> u64 {
        states
            .iter()
            .fold(0xcbf2_9ce4_8422_2325_u64, |hash, state| {
                (hash ^ u64::from((state >> 4) as u8)).wrapping_mul(0x100_0000_01b3)
            })
    }

    fn material_counts(states: &[u16]) -> (usize, usize, usize) {
        let mut stone = 0;
        let mut water = 0;
        let mut air = 0;
        for state in states {
            match *state {
                STONE_STATE => stone += 1,
                STILL_WATER_STATE => water += 1,
                AIR_STATE => air += 1,
                other => panic!("unexpected base terrain state {other}"),
            }
        }
        (stone, water, air)
    }

    #[test]
    fn independent_base_terrain_fixtures_match() {
        let fixtures = [
            (0, 0, 0, 0xbbc3_f085_459a_37ef_u64, (16_740, 0, 16_028)),
            (1, 0, 0, 0x368a_421e_0068_706d_u64, (12_611, 3_517, 16_640)),
            (-1, -1, -1, 0x4c72_fbd3_70ec_feeb_u64, (17_066, 0, 15_702)),
            (
                i32::MIN,
                7,
                -9,
                0xcea8_5b03_f4dc_efa2_u64,
                (18_335, 0, 14_433),
            ),
            (
                0x1234_5678,
                64,
                -128,
                0xe197_3c42_ba87_2265_u64,
                (16_604, 0, 16_164),
            ),
        ];

        for (seed, chunk_x, chunk_z, expected_hash, expected_counts) in fixtures {
            let shape =
                OverworldTerrainShape::new(seed).generate(ChunkCoord::new(chunk_x, chunk_z));
            assert_eq!(shape.states().len(), CHUNK_BLOCK_COUNT);
            assert_eq!(
                block_id_hash(shape.states()),
                expected_hash,
                "seed={seed} chunk={chunk_x}:{chunk_z}",
            );
            assert_eq!(
                material_counts(shape.states()),
                expected_counts,
                "seed={seed} chunk={chunk_x}:{chunk_z}",
            );
        }
    }

    #[test]
    fn base_stage_contains_only_air_stone_and_water() {
        let shape = OverworldTerrainShape::new(42).generate(ChunkCoord::new(-17, 23));
        assert!(
            shape
                .states()
                .iter()
                .all(|state| { matches!(*state, AIR_STATE | STONE_STATE | STILL_WATER_STATE) })
        );
        assert!(!shape.states().iter().any(|state| (*state >> 4) == 7));
    }
}
