const LCG_A: i64 = 6_364_136_223_846_793_005;
const LCG_B: i64 = 1_442_695_040_888_963_407;

#[derive(Clone, Copy)]
pub(super) struct LayerRandom {
    start_salt: i64,
}

impl LayerRandom {
    pub(super) fn new(world_seed: i64, seed_mixup: i64) -> Self {
        let mut layer_salt = seed_mixup;
        for _ in 0..3 {
            layer_salt = mix(layer_salt, seed_mixup);
        }

        let mut start_salt = world_seed;
        for _ in 0..3 {
            start_salt = mix(start_salt, layer_salt);
        }

        Self { start_salt }
    }

    /// The fixed-target Hills auxiliary zoom branch is not recursively initialized in this
    /// pre-1.13 layer graph. Its start salt/seed therefore remain zero.
    pub(super) const fn unseeded() -> Self {
        Self { start_salt: 0 }
    }

    pub(super) fn at(self, x: i32, z: i32) -> ChunkRandom {
        // Equivalent to startSeed = step(startSalt, 0), then startSeed + x followed by z/x/z.
        // Folding the first two operations into step(startSalt, x) preserves the exact bits.
        let mut seed = self.start_salt;
        seed = mix(seed, i64::from(x));
        seed = mix(seed, i64::from(z));
        seed = mix(seed, i64::from(x));
        seed = mix(seed, i64::from(z));
        ChunkRandom {
            seed,
            start_salt: self.start_salt,
        }
    }
}

pub(super) struct ChunkRandom {
    seed: i64,
    start_salt: i64,
}

impl ChunkRandom {
    pub(super) fn next(&mut self, bound: i32) -> i32 {
        debug_assert!(bound > 0);
        let mut value = (self.seed >> 24) % i64::from(bound);
        if value < 0 {
            value += i64::from(bound);
        }
        self.seed = mix(self.seed, self.start_salt);
        value as i32
    }
}

fn mix(state: i64, add: i64) -> i64 {
    state
        .wrapping_mul(state.wrapping_mul(LCG_A).wrapping_add(LCG_B))
        .wrapping_add(add)
}

pub(super) fn add_coord(base: i32, offset: usize) -> i32 {
    base.wrapping_add(offset as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_coordinates_and_wrapping_are_repeatable() {
        let layer = LayerRandom::new(i64::from(u32::MAX), 1001);
        let mut a = layer.at(-1, i32::MIN);
        let mut b = layer.at(-1, i32::MIN);
        for bound in [2, 3, 5, 13, 29, 57, 1024] {
            assert_eq!(a.next(bound), b.next(bound));
        }
    }

    #[test]
    fn unseeded_layer_keeps_zero_start_state() {
        let mut a = LayerRandom::unseeded().at(0, 0);
        let mut b = LayerRandom::unseeded().at(0, 0);
        for bound in [2, 3, 4] {
            assert_eq!(a.next(bound), b.next(bound));
        }
    }
}
