use std::sync::Arc;

use super::random::LayerRandom;

pub(super) const OCEAN: i32 = 0;
pub(super) const PLAINS: i32 = 1;
pub(super) const DESERT: i32 = 2;
pub(super) const EXTREME_HILLS: i32 = 3;
pub(super) const FOREST: i32 = 4;
pub(super) const TAIGA: i32 = 5;
pub(super) const SWAMPLAND: i32 = 6;
pub(super) const RIVER: i32 = 7;
pub(super) const FROZEN_OCEAN: i32 = 10;
pub(super) const FROZEN_RIVER: i32 = 11;
pub(super) const ICE_PLAINS: i32 = 12;
pub(super) const ICE_MOUNTAINS: i32 = 13;
pub(super) const MUSHROOM_ISLAND: i32 = 14;
pub(super) const MUSHROOM_SHORE: i32 = 15;
pub(super) const BEACH: i32 = 16;
pub(super) const DESERT_HILLS: i32 = 17;
pub(super) const FOREST_HILLS: i32 = 18;
pub(super) const TAIGA_HILLS: i32 = 19;
pub(super) const EXTREME_HILLS_EDGE: i32 = 20;
pub(super) const JUNGLE: i32 = 21;
pub(super) const JUNGLE_HILLS: i32 = 22;
pub(super) const JUNGLE_EDGE: i32 = 23;
pub(super) const DEEP_OCEAN: i32 = 24;
pub(super) const STONE_BEACH: i32 = 25;
pub(super) const COLD_BEACH: i32 = 26;
pub(super) const BIRCH_FOREST: i32 = 27;
pub(super) const BIRCH_FOREST_HILLS: i32 = 28;
pub(super) const ROOFED_FOREST: i32 = 29;
pub(super) const COLD_TAIGA: i32 = 30;
pub(super) const COLD_TAIGA_HILLS: i32 = 31;
pub(super) const MEGA_TAIGA: i32 = 32;
pub(super) const MEGA_TAIGA_HILLS: i32 = 33;
pub(super) const EXTREME_HILLS_PLUS: i32 = 34;
pub(super) const SAVANNA: i32 = 35;
pub(super) const SAVANNA_PLATEAU: i32 = 36;
pub(super) const MESA: i32 = 37;
pub(super) const MESA_PLATEAU_F: i32 = 38;
pub(super) const MESA_PLATEAU: i32 = 39;

pub(super) const CLIMATE_WARM: i32 = 1;
pub(super) const CLIMATE_LUSH: i32 = 2;
pub(super) const CLIMATE_COLD: i32 = 3;
pub(super) const CLIMATE_FREEZING: i32 = 4;

pub(super) type LayerRef = Arc<Layer>;

#[derive(Clone, Copy)]
pub(super) enum EdgeMode {
    CoolWarm,
    HeatIce,
    Special,
}

#[derive(Clone)]
pub(super) enum LayerKind {
    Island,
    Zoom {
        parent: LayerRef,
        fuzzy: bool,
    },
    AddIsland(LayerRef),
    RemoveTooMuchOcean(LayerRef),
    AddSnow(LayerRef),
    AddEdge {
        parent: LayerRef,
        mode: EdgeMode,
    },
    AddMushroomIsland(LayerRef),
    DeepOcean(LayerRef),
    RiverInit(LayerRef),
    BiomeInit(LayerRef),
    BiomeEdge(LayerRef),
    RegionHills {
        parent: LayerRef,
        river_noise: LayerRef,
    },
    RareBiomeSpot(LayerRef),
    River(LayerRef),
    Smooth(LayerRef),
    Shore(LayerRef),
    RiverMixer {
        biomes: LayerRef,
        rivers: LayerRef,
    },
    VoronoiZoom(LayerRef),
}

#[derive(Clone)]
pub(super) struct Layer {
    pub(super) random: LayerRandom,
    kind: LayerKind,
}

impl Layer {
    pub(super) fn new(world_seed: i64, seed_mixup: i64, kind: LayerKind) -> LayerRef {
        Arc::new(Self {
            random: LayerRandom::new(world_seed, seed_mixup),
            kind,
        })
    }

    pub(super) fn new_unseeded(kind: LayerKind) -> LayerRef {
        Arc::new(Self {
            random: LayerRandom::unseeded(),
            kind,
        })
    }

    pub(super) fn area(&self, x: i32, z: i32, width: usize, height: usize) -> Vec<i32> {
        match &self.kind {
            LayerKind::Island => self.island(x, z, width, height),
            LayerKind::Zoom { parent, fuzzy } => self.zoom(parent, *fuzzy, x, z, width, height),
            LayerKind::AddIsland(parent) => self.add_island(parent, x, z, width, height),
            LayerKind::RemoveTooMuchOcean(parent) => {
                self.remove_too_much_ocean(parent, x, z, width, height)
            }
            LayerKind::AddSnow(parent) => self.add_snow(parent, x, z, width, height),
            LayerKind::AddEdge { parent, mode } => {
                self.add_edge(parent, *mode, x, z, width, height)
            }
            LayerKind::AddMushroomIsland(parent) => {
                self.add_mushroom_island(parent, x, z, width, height)
            }
            LayerKind::DeepOcean(parent) => self.deep_ocean(parent, x, z, width, height),
            LayerKind::RiverInit(parent) => self.river_init(parent, x, z, width, height),
            LayerKind::BiomeInit(parent) => self.biome_init(parent, x, z, width, height),
            LayerKind::BiomeEdge(parent) => self.biome_edge(parent, x, z, width, height),
            LayerKind::RegionHills {
                parent,
                river_noise,
            } => self.region_hills(parent, river_noise, x, z, width, height),
            LayerKind::RareBiomeSpot(parent) => self.rare_biome_spot(parent, x, z, width, height),
            LayerKind::River(parent) => self.river(parent, x, z, width, height),
            LayerKind::Smooth(parent) => self.smooth(parent, x, z, width, height),
            LayerKind::Shore(parent) => self.shore(parent, x, z, width, height),
            LayerKind::RiverMixer { biomes, rivers } => {
                self.river_mixer(biomes, rivers, x, z, width, height)
            }
            LayerKind::VoronoiZoom(parent) => self.voronoi(parent, x, z, width, height),
        }
    }
}

pub(super) fn neighborhood(
    parent: &LayerRef,
    x: i32,
    z: i32,
    width: usize,
    height: usize,
    mut project: impl FnMut(i32, i32, i32, i32, i32, i32, i32) -> i32,
) -> Vec<i32> {
    let stride = width + 2;
    let input = parent.area(x.wrapping_sub(1), z.wrapping_sub(1), stride, height + 2);
    let mut out = vec![0; width * height];

    for dz in 0..height {
        for dx in 0..width {
            let center = input[dx + 1 + (dz + 1) * stride];
            let north = input[dx + 1 + dz * stride];
            let east = input[dx + 2 + (dz + 1) * stride];
            let west = input[dx + (dz + 1) * stride];
            let south = input[dx + 1 + (dz + 2) * stride];
            out[dx + dz * width] = project(
                center,
                north,
                east,
                west,
                south,
                super::random::add_coord(x, dx),
                super::random::add_coord(z, dz),
            );
        }
    }

    out
}

pub(super) fn is_oceanic(id: i32) -> bool {
    matches!(id, OCEAN | FROZEN_OCEAN | DEEP_OCEAN)
}

pub(super) fn is_shallow_ocean(id: i32) -> bool {
    matches!(id, OCEAN | FROZEN_OCEAN)
}
