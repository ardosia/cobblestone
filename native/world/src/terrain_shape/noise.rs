const MT_N: usize = 624;
const MT_M: usize = 397;
const MATRIX_A: u32 = 0x9908_b0df;
const UPPER_MASK: u32 = 0x8000_0000;
const LOWER_MASK: u32 = 0x7fff_ffff;

pub(super) struct MtRandom {
    state: [u32; MT_N],
    index: usize,
}

impl MtRandom {
    pub(super) fn new(seed: u32) -> Self {
        let mut state = [0_u32; MT_N];
        state[0] = seed;
        for index in 1..MT_N {
            let previous = state[index - 1];
            state[index] = 1_812_433_253_u32
                .wrapping_mul(previous ^ (previous >> 30))
                .wrapping_add(index as u32);
        }
        Self { state, index: MT_N }
    }

    pub(super) fn next_u32(&mut self) -> u32 {
        if self.index > MT_N {
            *self = Self::new(5489);
            self.index = 0;
        } else if self.index == MT_N {
            self.index = 0;
        }

        let index = self.index;
        if index < MT_N - MT_M {
            let y = (self.state[index] & UPPER_MASK) | (self.state[index + 1] & LOWER_MASK);
            self.state[index] =
                self.state[index + MT_M] ^ (y >> 1) ^ if y & 1 == 0 { 0 } else { MATRIX_A };
        } else if index < MT_N - 1 {
            let y = (self.state[index] & UPPER_MASK) | (self.state[index + 1] & LOWER_MASK);
            self.state[index] =
                self.state[index + MT_M - MT_N] ^ (y >> 1) ^ if y & 1 == 0 { 0 } else { MATRIX_A };
        } else {
            let y = (self.state[MT_N - 1] & UPPER_MASK) | (self.state[0] & LOWER_MASK);
            self.state[MT_N - 1] =
                self.state[MT_M - 1] ^ (y >> 1) ^ if y & 1 == 0 { 0 } else { MATRIX_A };
        }
        self.index += 1;

        let mut value = self.state[index];
        value ^= value >> 11;
        value ^= (value << 7) & 0x9d2c_5680;
        value ^= (value << 15) & 0xefc6_0000;
        value ^= value >> 18;
        value
    }

    pub(super) fn next_float(&mut self) -> f32 {
        (f64::from(self.next_u32()) * (1.0 / 4_294_967_296.0)) as f32
    }

    pub(super) fn next_int(&mut self, bound: u32) -> u32 {
        debug_assert!(bound > 0);
        self.next_u32() % bound
    }
}

pub(super) struct PerlinNoise {
    levels: Vec<ImprovedNoise>,
}

impl PerlinNoise {
    pub(super) fn new(random: &mut MtRandom, levels: usize) -> Self {
        Self {
            levels: (0..levels).map(|_| ImprovedNoise::new(random)).collect(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn region(
        &self,
        x: f32,
        y: f32,
        z: f32,
        x_size: usize,
        y_size: usize,
        z_size: usize,
        x_scale: f32,
        y_scale: f32,
        z_scale: f32,
    ) -> Vec<f32> {
        let mut out = vec![0.0; x_size * y_size * z_size];
        let mut octave_scale = 1.0_f32;

        for level in &self.levels {
            level.add_region(
                &mut out,
                x,
                y,
                z,
                x_size,
                y_size,
                z_size,
                x_scale * octave_scale,
                y_scale * octave_scale,
                z_scale * octave_scale,
                octave_scale,
            );
            octave_scale *= 0.5;
        }

        out
    }
}

/// The fixed-target constructor creates four surface-simplex octaves before scale/depth Perlin
/// banks. Base terrain does not read them, but their initialization consumes the shared MT stream.
pub(super) fn consume_simplex_initialization(random: &mut MtRandom, levels: usize) {
    for _ in 0..levels {
        random.next_float();
        random.next_float();
        random.next_float();

        let mut permutation = [0_u16; 256];
        for (index, value) in permutation.iter_mut().enumerate() {
            *value = index as u16;
        }
        for index in 0..256 {
            let offset = random.next_int((256 - index) as u32) as usize;
            permutation.swap(index, index + offset);
        }
    }
}

struct ImprovedNoise {
    origin_x: f32,
    origin_y: f32,
    origin_z: f32,
    permutation: [u16; 512],
}

impl ImprovedNoise {
    fn new(random: &mut MtRandom) -> Self {
        let origin_x = random.next_float() * 256.0;
        let origin_y = random.next_float() * 256.0;
        let origin_z = random.next_float() * 256.0;

        let mut permutation = [0_u16; 512];
        for (index, value) in permutation[..256].iter_mut().enumerate() {
            *value = index as u16;
        }

        for index in 0..256 {
            let swap = index + random.next_int((256 - index) as u32) as usize;
            permutation.swap(index, swap);
            permutation[index + 256] = permutation[index];
        }

        Self {
            origin_x,
            origin_y,
            origin_z,
            permutation,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn add_region(
        &self,
        out: &mut [f32],
        x: f32,
        y: f32,
        z: f32,
        x_size: usize,
        y_size: usize,
        z_size: usize,
        x_scale: f32,
        y_scale: f32,
        z_scale: f32,
        octave_scale: f32,
    ) {
        if y_size == 1 {
            self.add_region_2d(out, x, z, x_size, z_size, x_scale, z_scale, octave_scale);
            return;
        }

        let amplitude = 1.0 / octave_scale;
        let mut out_index = 0;

        for dx in 0..x_size {
            let (x_local, x_index, x_fade) = calc_values((x + dx as f32) * x_scale + self.origin_x);

            for dz in 0..z_size {
                let (z_local, z_index, z_fade) =
                    calc_values((z + dz as f32) * z_scale + self.origin_z);

                let mut previous_y = usize::MAX;
                let (mut corner_0, mut corner_1, mut corner_2, mut corner_3) = (0.0, 0.0, 0.0, 0.0);

                for dy in 0..y_size {
                    let (y_local, y_index, y_fade) =
                        calc_values((y + dy as f32) * y_scale + self.origin_y);

                    if dy == 0 || y_index != previous_y {
                        previous_y = y_index;
                        (corner_0, corner_1, corner_2, corner_3) = self.blend_cube_corners(
                            x_local, y_local, z_local, x_index, y_index, z_index, x_fade,
                        );
                    }

                    let near = lerp(y_fade, corner_0, corner_1);
                    let far = lerp(y_fade, corner_2, corner_3);
                    out[out_index] += lerp(z_fade, near, far) * amplitude;
                    out_index += 1;
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn add_region_2d(
        &self,
        out: &mut [f32],
        x: f32,
        z: f32,
        x_size: usize,
        z_size: usize,
        x_scale: f32,
        z_scale: f32,
        octave_scale: f32,
    ) {
        let amplitude = 1.0 / octave_scale;
        let mut out_index = 0;

        for dx in 0..x_size {
            let (x_local, x_index, x_fade) = calc_values((x + dx as f32) * x_scale + self.origin_x);

            for dz in 0..z_size {
                let (z_local, z_index, z_fade) =
                    calc_values((z + dz as f32) * z_scale + self.origin_z);

                let a = usize::from(self.permutation[x_index]);
                let aa = usize::from(self.permutation[a]) + z_index;
                let b = usize::from(self.permutation[x_index + 1]);
                let ba = usize::from(self.permutation[b]) + z_index;

                let near = lerp(
                    x_fade,
                    grad2(self.permutation[aa], x_local, z_local),
                    grad(self.permutation[ba], x_local - 1.0, 0.0, z_local),
                );
                let far = lerp(
                    x_fade,
                    grad(self.permutation[aa + 1], x_local, 0.0, z_local - 1.0),
                    grad(self.permutation[ba + 1], x_local - 1.0, 0.0, z_local - 1.0),
                );

                out[out_index] += lerp(z_fade, near, far) * amplitude;
                out_index += 1;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn blend_cube_corners(
        &self,
        x: f32,
        y: f32,
        z: f32,
        x_index: usize,
        y_index: usize,
        z_index: usize,
        x_fade: f32,
    ) -> (f32, f32, f32, f32) {
        let a = usize::from(self.permutation[x_index]) + y_index;
        let aa = usize::from(self.permutation[a]) + z_index;
        let ab = usize::from(self.permutation[a + 1]) + z_index;
        let b = usize::from(self.permutation[x_index + 1]) + y_index;
        let ba = usize::from(self.permutation[b]) + z_index;
        let bb = usize::from(self.permutation[b + 1]) + z_index;

        (
            lerp(
                x_fade,
                grad(self.permutation[aa], x, y, z),
                grad(self.permutation[ba], x - 1.0, y, z),
            ),
            lerp(
                x_fade,
                grad(self.permutation[ab], x, y - 1.0, z),
                grad(self.permutation[bb], x - 1.0, y - 1.0, z),
            ),
            lerp(
                x_fade,
                grad(self.permutation[aa + 1], x, y, z - 1.0),
                grad(self.permutation[ba + 1], x - 1.0, y, z - 1.0),
            ),
            lerp(
                x_fade,
                grad(self.permutation[ab + 1], x, y - 1.0, z - 1.0),
                grad(self.permutation[bb + 1], x - 1.0, y - 1.0, z - 1.0),
            ),
        )
    }
}

fn calc_values(mut value: f32) -> (f32, usize, f32) {
    let mut floor = value as i32;
    if value < floor as f32 {
        floor -= 1;
    }
    let index = (floor & 255) as usize;
    value -= floor as f32;
    let fade = value * value * value * (value * (value * 6.0 - 15.0) + 10.0);
    (value, index, fade)
}

fn lerp(t: f32, a: f32, b: f32) -> f32 {
    a + t * (b - a)
}

fn grad2(hash: u16, x: f32, z: f32) -> f32 {
    let h = hash & 15;
    let u = (1 - ((h & 8) >> 3)) as f32 * x;
    let v = if h < 4 {
        0.0
    } else if h == 12 || h == 14 {
        x
    } else {
        z
    };
    (if h & 1 == 0 { u } else { -u }) + if h & 2 == 0 { v } else { -v }
}

fn grad(hash: u16, x: f32, y: f32, z: f32) -> f32 {
    let h = hash & 15;
    let u = if h < 8 { x } else { y };
    let v = if h < 4 {
        y
    } else if h == 12 || h == 14 {
        x
    } else {
        z
    };
    (if h & 1 == 0 { u } else { -u }) + if h & 2 == 0 { v } else { -v }
}

#[cfg(test)]
mod tests {
    use super::MtRandom;

    #[test]
    fn target_mt_seed_prefixes_match_mcpe_reference() {
        let fixtures = [
            (
                0_u32,
                [
                    0x8c7f_0aac,
                    0x97c4_aa2f,
                    0xb716_a675,
                    0xd821_ccc0,
                    0x9a4e_b343,
                    0xdba2_52fb,
                    0x8b7d_76c3,
                    0xd8e5_7d67,
                ],
            ),
            (
                u32::MAX,
                [
                    0x18fe_69a3,
                    0x1c92_4122,
                    0xe991_ec0c,
                    0x900c_ac47,
                    0xc9fe_37b4,
                    0x86bc_fe40,
                    0xc7ae_50d6,
                    0xc547_01fa,
                ],
            ),
            (
                0x8000_0000,
                [
                    0x26e9_a91a,
                    0x55d4_1404,
                    0xd20f_1711,
                    0x5c53_5c23,
                    0xc687_0e45,
                    0x2989_4a09,
                    0x8a83_b4b4,
                    0x664c_9d9d,
                ],
            ),
        ];

        for (seed, expected) in fixtures {
            let mut random = MtRandom::new(seed);
            let actual = std::array::from_fn(|_| random.next_u32());
            assert_eq!(actual, expected, "seed=0x{seed:08x}");
        }
    }
}
