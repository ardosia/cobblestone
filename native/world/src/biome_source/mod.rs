mod biome_layers;
mod climate_layers;
mod layer;
mod random;
mod zoom_layers;

use std::sync::Arc;

use crate::{ChunkCoord, default_biome_word};

use layer::{EdgeMode, Layer, LayerKind, LayerRef};

const PLAYER_VALID_SPAWNS: [u8; 7] = [4, 1, 5, 19, 18, 21, 22];

pub struct OverworldBiomeSource {
    raw_layer: LayerRef,
    final_layer: LayerRef,
}

impl OverworldBiomeSource {
    pub fn new(seed: i32) -> Self {
        // Target BiomeSource accepts the world seed in a 32-bit register and zero-extends the
        // exact two's-complement bits into the recovered 64-bit layer RNG.
        let world_seed = i64::from(u32::from_ne_bytes(seed.to_ne_bytes()));

        let mut base = Layer::new(world_seed, 1, LayerKind::Island);
        base = Layer::new(
            world_seed,
            2000,
            LayerKind::Zoom {
                parent: base,
                fuzzy: true,
            },
        );
        base = Layer::new(world_seed, 1, LayerKind::AddIsland(base));
        base = zoom(world_seed, base, 2001);
        base = Layer::new(world_seed, 2, LayerKind::AddIsland(base));
        base = Layer::new(world_seed, 50, LayerKind::AddIsland(base));
        base = Layer::new(world_seed, 70, LayerKind::AddIsland(base));
        base = Layer::new(world_seed, 2, LayerKind::RemoveTooMuchOcean(base));
        base = Layer::new(world_seed, 2, LayerKind::AddSnow(base));
        base = Layer::new(world_seed, 3, LayerKind::AddIsland(base));
        base = Layer::new(
            world_seed,
            2,
            LayerKind::AddEdge {
                parent: base,
                mode: EdgeMode::CoolWarm,
            },
        );
        base = Layer::new(
            world_seed,
            2,
            LayerKind::AddEdge {
                parent: base,
                mode: EdgeMode::HeatIce,
            },
        );
        base = Layer::new(
            world_seed,
            3,
            LayerKind::AddEdge {
                parent: base,
                mode: EdgeMode::Special,
            },
        );
        base = zoom(world_seed, base, 2002);
        base = zoom(world_seed, base, 2003);
        base = Layer::new(world_seed, 4, LayerKind::AddIsland(base));
        base = Layer::new(world_seed, 5, LayerKind::AddMushroomIsland(base));
        base = Layer::new(world_seed, 4, LayerKind::DeepOcean(base));

        let river_init = Layer::new(world_seed, 100, LayerKind::RiverInit(Arc::clone(&base)));
        // Fixed-target graph quirk: these two Hills auxiliary zoom layers are not reached by
        // recursive world-seed initialization. Their own random state stays zero while the
        // shared RiverInit parent is normally seeded.
        let hills_noise = zoom_unseeded(zoom_unseeded(Arc::clone(&river_init)));

        let mut river = zoom_times(world_seed, Arc::clone(&river_init), 1000, 2);
        river = zoom_times(world_seed, river, 1000, 4);
        river = Layer::new(world_seed, 1, LayerKind::River(river));
        river = Layer::new(world_seed, 1000, LayerKind::Smooth(river));

        let mut biomes = Layer::new(world_seed, 200, LayerKind::BiomeInit(base));
        biomes = zoom_times(world_seed, biomes, 1000, 2);
        biomes = Layer::new(world_seed, 1000, LayerKind::BiomeEdge(biomes));
        biomes = Layer::new(
            world_seed,
            1000,
            LayerKind::RegionHills {
                parent: biomes,
                river_noise: hills_noise,
            },
        );
        biomes = Layer::new(world_seed, 1001, LayerKind::RareBiomeSpot(biomes));

        for index in 0..4_i64 {
            biomes = zoom(world_seed, biomes, 1000 + index);
            if index == 0 {
                biomes = Layer::new(world_seed, 3, LayerKind::AddIsland(biomes));
            }
            if index == 1 {
                biomes = Layer::new(world_seed, 1000, LayerKind::Shore(biomes));
            }
        }

        biomes = Layer::new(world_seed, 1000, LayerKind::Smooth(biomes));
        let mixed = Layer::new(
            world_seed,
            100,
            LayerKind::RiverMixer {
                biomes,
                rivers: river,
            },
        );
        let final_layer = Layer::new(world_seed, 10, LayerKind::VoronoiZoom(Arc::clone(&mixed)));

        Self {
            raw_layer: mixed,
            final_layer,
        }
    }

    pub fn biome_ids(&self, x: i32, z: i32, width: usize, height: usize) -> Vec<u8> {
        self.final_layer
            .area(x, z, width, height)
            .into_iter()
            .map(|id| {
                u8::try_from(id)
                    .ok()
                    .filter(|value| crate::biome_id_is_supported(*value))
                    .unwrap_or_else(|| {
                        panic!("biome source emitted unsupported fixed-target id {id}")
                    })
            })
            .collect()
    }

    pub(crate) fn raw_biome_ids(&self, x: i32, z: i32, width: usize, height: usize) -> Vec<u8> {
        self.raw_layer
            .area(x, z, width, height)
            .into_iter()
            .map(|id| {
                u8::try_from(id)
                    .ok()
                    .filter(|value| crate::biome_id_is_supported(*value))
                    .unwrap_or_else(|| {
                        panic!("raw biome source emitted unsupported fixed-target id {id}")
                    })
            })
            .collect()
    }

    pub fn chunk_words(&self, position: ChunkCoord) -> Vec<u32> {
        let x = position.x.wrapping_mul(16);
        let z = position.z.wrapping_mul(16);
        self.biome_ids(x, z, 16, 16)
            .into_iter()
            .map(|id| default_biome_word(id).expect("source only emits registered biomes"))
            .collect()
    }

    /// Fixed-target BiomeSource::getSpawnPosition X/Z search.
    ///
    /// The target stores an invalid Y sentinel for Player-side height resolution. Cobblestone
    /// exposes only the deterministic X/Z here; the current PHP session layer supplies its own
    /// safe temporary Y until Player spawn-height semantics are implemented.
    pub fn spawn_position(&self) -> (i32, i32) {
        let mut x = 0_i32;
        loop {
            // The restored target asks a 10x10 raw-biome area and probes the four neighbors of
            // each candidate. Sample one explicit padding cell on every side so those probes are
            // represented without relying on target LayerData scratch-buffer padding.
            let raw_x = x / 4;
            let raw = self.raw_layer.area(raw_x - 1, -1, 12, 12);

            for zo in 0..10_usize {
                for xo in 0..10_usize {
                    let cx = xo + 1;
                    let cz = zo + 1;
                    let index = cx + cz * 12;
                    let valid = [
                        raw[index],
                        raw[index - 1],
                        raw[index + 1],
                        raw[index - 12],
                        raw[index + 12],
                    ]
                    .into_iter()
                    .all(|id| {
                        u8::try_from(id)
                            .ok()
                            .is_some_and(|id| PLAYER_VALID_SPAWNS.contains(&id))
                    });
                    if valid {
                        return (
                            x.wrapping_add((xo as i32).wrapping_mul(4)),
                            (zo as i32).wrapping_mul(4),
                        );
                    }
                }
            }
            x = x.wrapping_add(40);
        }
    }
}

fn zoom_unseeded(parent: LayerRef) -> LayerRef {
    Layer::new_unseeded(LayerKind::Zoom {
        parent,
        fuzzy: false,
    })
}

fn zoom(world_seed: i64, parent: LayerRef, seed_mixup: i64) -> LayerRef {
    Layer::new(
        world_seed,
        seed_mixup,
        LayerKind::Zoom {
            parent,
            fuzzy: false,
        },
    )
}

fn zoom_times(world_seed: i64, mut parent: LayerRef, seed_mixup: i64, count: usize) -> LayerRef {
    for index in 0..count {
        parent = zoom(world_seed, parent, seed_mixup + index as i64);
    }
    parent
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash_ids(ids: &[u8]) -> u64 {
        ids.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
        })
    }

    #[test]
    fn overworld_source_is_deterministic_across_seed_and_coordinate_boundaries() {
        for seed in [0, 1, -1, i32::MIN, i32::MAX, 0x1234_5678] {
            let source = OverworldBiomeSource::new(seed);
            for (x, z) in [(0, 0), (-1, -1), (-16, 31), (1024, -2048)] {
                let first = source.biome_ids(x, z, 16, 16);
                let second = source.biome_ids(x, z, 16, 16);
                assert_eq!(first, second, "seed={seed} at {x}:{z}");
                assert_eq!(first.len(), 256);
                assert!(first.iter().all(|id| crate::biome_id_is_supported(*id)));
            }
        }
    }

    #[test]
    fn chunk_words_use_catalog_default_colors() {
        let source = OverworldBiomeSource::new(42);
        let ids = source.biome_ids(0, 0, 16, 16);
        let words = source.chunk_words(ChunkCoord::new(0, 0));
        assert_eq!(words.len(), 256);
        for (id, word) in ids.into_iter().zip(words) {
            assert_eq!(word >> 24, u32::from(id));
            assert_eq!(Some(word), default_biome_word(id));
        }
    }

    #[test]
    fn spawn_search_returns_target_valid_raw_biome_cross() {
        for seed in [0, 1, -1, i32::MIN, i32::MAX, 0x1234_5678] {
            let source = OverworldBiomeSource::new(seed);
            let (x, z) = source.spawn_position();
            assert_eq!(x.rem_euclid(4), 0, "seed={seed}");
            assert_eq!(z.rem_euclid(4), 0, "seed={seed}");

            let raw = source.raw_biome_ids(x / 4 - 1, z / 4 - 1, 3, 3);
            for index in [4_usize, 3, 5, 1, 7] {
                assert!(
                    PLAYER_VALID_SPAWNS.contains(&raw[index]),
                    "seed={seed} spawn={x}:{z} raw={:?}",
                    raw
                );
            }
        }
    }

    #[test]
    fn representative_area_hashes_match_independent_java_1_8_layer_oracle() {
        // Derived independently with cubiomes MC_1_8 after the target binary established the
        // layer graph/classes/seeds. These are fixed fixtures, not a runtime dependency.
        let fixtures = [
            (0, 0, 0, 16, 16, 0x4dc7_20c3_77aa_3b25_u64),
            (1, 0, 0, 16, 16, 0xd80a_c658_736b_b725_u64),
            (-1, -16, -16, 16, 16, 0x19d4_de88_1f0a_3b25_u64),
            (0x1234_5678, 1024, -2048, 16, 16, 0x1ae8_1315_307d_5025_u64),
            (i32::MIN, -3, 5, 17, 19, 0x6f95_6c37_652f_c0c3_u64),
            (i32::MAX, 31, -33, 7, 11, 0x8a01_af81_74bb_582f_u64),
            (42, -257, 258, 13, 9, 0x40b1_04e8_91c2_9a6b_u64),
        ];
        for (seed, x, z, width, height, expected) in fixtures {
            let actual = hash_ids(&OverworldBiomeSource::new(seed).biome_ids(x, z, width, height));
            assert_eq!(
                actual, expected,
                "seed={seed} at {x}:{z} size={width}x{height}",
            );
        }
    }
}
