use super::layer::{
    BEACH, BIRCH_FOREST, BIRCH_FOREST_HILLS, CLIMATE_COLD, CLIMATE_FREEZING, CLIMATE_LUSH,
    CLIMATE_WARM, COLD_BEACH, COLD_TAIGA, COLD_TAIGA_HILLS, DEEP_OCEAN, DESERT, DESERT_HILLS,
    EXTREME_HILLS, EXTREME_HILLS_EDGE, EXTREME_HILLS_PLUS, FOREST, FOREST_HILLS, FROZEN_OCEAN,
    FROZEN_RIVER, ICE_MOUNTAINS, ICE_PLAINS, JUNGLE, JUNGLE_EDGE, JUNGLE_HILLS, Layer, LayerRef,
    MEGA_TAIGA, MEGA_TAIGA_HILLS, MESA, MESA_PLATEAU, MESA_PLATEAU_F, MUSHROOM_ISLAND,
    MUSHROOM_SHORE, OCEAN, PLAINS, RIVER, ROOFED_FOREST, SAVANNA, SAVANNA_PLATEAU, STONE_BEACH,
    SWAMPLAND, TAIGA, TAIGA_HILLS, is_oceanic, is_shallow_ocean, neighborhood,
};
use super::random::add_coord;

impl Layer {
    pub(super) fn river_init(
        &self,
        parent: &LayerRef,
        x: i32,
        z: i32,
        width: usize,
        height: usize,
    ) -> Vec<i32> {
        parent
            .area(x, z, width, height)
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                if value > OCEAN {
                    let dx = index % width;
                    let dz = index / width;
                    self.random
                        .at(add_coord(x, dx), add_coord(z, dz))
                        .next(299_999)
                        + 2
                } else {
                    OCEAN
                }
            })
            .collect()
    }

    pub(super) fn biome_init(
        &self,
        parent: &LayerRef,
        x: i32,
        z: i32,
        width: usize,
        height: usize,
    ) -> Vec<i32> {
        const HOT: [i32; 6] = [DESERT, DESERT, DESERT, SAVANNA, SAVANNA, PLAINS];
        // Fixed-target MCPE weights Plains three times in the medium climate list.
        // This differs from the Java 1.8 six-entry table.
        const LUSH: [i32; 8] = [
            FOREST,
            ROOFED_FOREST,
            EXTREME_HILLS,
            PLAINS,
            PLAINS,
            PLAINS,
            BIRCH_FOREST,
            SWAMPLAND,
        ];
        const COLD: [i32; 4] = [FOREST, EXTREME_HILLS, TAIGA, PLAINS];
        const FREEZING: [i32; 4] = [ICE_PLAINS, ICE_PLAINS, ICE_PLAINS, COLD_TAIGA];

        parent
            .area(x, z, width, height)
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                let special = (value >> 8) & 0x0f;
                let climate = value & !0x0f00;
                if is_oceanic(climate) || climate == MUSHROOM_ISLAND {
                    return climate;
                }

                let dx = index % width;
                let dz = index / width;
                let mut random = self.random.at(add_coord(x, dx), add_coord(z, dz));
                match climate {
                    CLIMATE_WARM if special == 0 => HOT[random.next(HOT.len() as i32) as usize],
                    CLIMATE_WARM => {
                        if random.next(3) == 0 {
                            MESA_PLATEAU
                        } else {
                            MESA_PLATEAU_F
                        }
                    }
                    CLIMATE_LUSH if special == 0 => LUSH[random.next(LUSH.len() as i32) as usize],
                    CLIMATE_LUSH => JUNGLE,
                    CLIMATE_COLD if special == 0 => COLD[random.next(COLD.len() as i32) as usize],
                    CLIMATE_COLD => MEGA_TAIGA,
                    CLIMATE_FREEZING => FREEZING[random.next(FREEZING.len() as i32) as usize],
                    other => other,
                }
            })
            .collect()
    }

    pub(super) fn biome_edge(
        &self,
        parent: &LayerRef,
        x: i32,
        z: i32,
        width: usize,
        height: usize,
    ) -> Vec<i32> {
        neighborhood(parent, x, z, width, height, |center, n, e, w, s, _, _| {
            let adjacent = [n, e, w, s];

            if let Some(edge) = replace_edge(center, &adjacent, MESA_PLATEAU_F, MESA) {
                return edge;
            }
            if let Some(edge) = replace_edge(center, &adjacent, MESA_PLATEAU, MESA) {
                return edge;
            }
            if let Some(edge) = replace_edge(center, &adjacent, MEGA_TAIGA, TAIGA) {
                return edge;
            }

            if center == DESERT {
                return if adjacent.contains(&ICE_PLAINS) {
                    EXTREME_HILLS_PLUS
                } else {
                    center
                };
            }

            if center == SWAMPLAND {
                if adjacent
                    .iter()
                    .any(|value| matches!(*value, DESERT | COLD_TAIGA | ICE_PLAINS))
                {
                    return PLAINS;
                }
                if adjacent.contains(&JUNGLE) {
                    return JUNGLE_EDGE;
                }
            }

            center
        })
    }

    pub(super) fn region_hills(
        &self,
        parent: &LayerRef,
        river_noise: &LayerRef,
        x: i32,
        z: i32,
        width: usize,
        height: usize,
    ) -> Vec<i32> {
        let stride = width + 2;
        let px = x.wrapping_sub(1);
        let pz = z.wrapping_sub(1);
        let biomes = parent.area(px, pz, stride, height + 2);
        let noise = river_noise.area(px, pz, stride, height + 2);
        let mut out = vec![0; width * height];

        for dz in 0..height {
            for dx in 0..width {
                let index = dx + 1 + (dz + 1) * stride;
                let biome = biomes[index];
                let noise_value = noise[index];
                let remainder = (noise_value - 2) % 29;
                let mutation_flag = remainder == 0;
                let out_index = dx + dz * width;

                if remainder == 1 && noise_value >= 2 && !is_shallow_ocean(biome) {
                    out[out_index] = mutated_biome(biome).unwrap_or(biome);
                    continue;
                }

                let mut random = self.random.at(add_coord(x, dx), add_coord(z, dz));
                // The first value is advanced even on the mutation-flag path in the recovered
                // pre-1.13 implementation before any biome-specific random branch is read.
                let first_choice = random.next(3);
                if !mutation_flag && first_choice != 0 {
                    out[out_index] = biome;
                    continue;
                }

                let mut hill = match biome {
                    DESERT => DESERT_HILLS,
                    FOREST => FOREST_HILLS,
                    BIRCH_FOREST => BIRCH_FOREST_HILLS,
                    ROOFED_FOREST => PLAINS,
                    TAIGA => TAIGA_HILLS,
                    MEGA_TAIGA => MEGA_TAIGA_HILLS,
                    COLD_TAIGA => COLD_TAIGA_HILLS,
                    PLAINS => {
                        if random.next(3) == 0 {
                            FOREST_HILLS
                        } else {
                            FOREST
                        }
                    }
                    ICE_PLAINS => ICE_MOUNTAINS,
                    JUNGLE => JUNGLE_HILLS,
                    OCEAN => DEEP_OCEAN,
                    EXTREME_HILLS => EXTREME_HILLS_PLUS,
                    SAVANNA => SAVANNA_PLATEAU,
                    MESA_PLATEAU_F | MESA_PLATEAU => MESA,
                    DEEP_OCEAN if random.next(3) == 0 => {
                        if random.next(2) == 0 {
                            PLAINS
                        } else {
                            FOREST
                        }
                    }
                    _ => biome,
                };

                if mutation_flag && hill != biome {
                    hill = mutated_biome(hill).unwrap_or(biome);
                }

                if hill == biome {
                    out[out_index] = biome;
                    continue;
                }

                let north = biomes[dx + 1 + dz * stride];
                let east = biomes[dx + 2 + (dz + 1) * stride];
                let west = biomes[dx + (dz + 1) * stride];
                let south = biomes[dx + 1 + (dz + 2) * stride];
                let similar = [north, east, west, south]
                    .iter()
                    .filter(|candidate| biome_similar(**candidate, biome))
                    .count();
                out[out_index] = if similar >= 3 { hill } else { biome };
            }
        }

        out
    }

    pub(super) fn rare_biome_spot(
        &self,
        parent: &LayerRef,
        x: i32,
        z: i32,
        width: usize,
        height: usize,
    ) -> Vec<i32> {
        parent
            .area(x, z, width, height)
            .into_iter()
            .enumerate()
            .map(|(index, biome)| {
                let dx = index % width;
                let dz = index / width;
                if biome == PLAINS
                    && self.random.at(add_coord(x, dx), add_coord(z, dz)).next(57) == 0
                {
                    129
                } else {
                    biome
                }
            })
            .collect()
    }

    pub(super) fn river(
        &self,
        parent: &LayerRef,
        x: i32,
        z: i32,
        width: usize,
        height: usize,
    ) -> Vec<i32> {
        neighborhood(parent, x, z, width, height, |center, n, e, w, s, _, _| {
            let center = reduce_river_id(center);
            let n = reduce_river_id(n);
            let e = reduce_river_id(e);
            let w = reduce_river_id(w);
            let s = reduce_river_id(s);
            if center == n && center == e && center == w && center == s {
                -1
            } else {
                RIVER
            }
        })
    }

    pub(super) fn smooth(
        &self,
        parent: &LayerRef,
        x: i32,
        z: i32,
        width: usize,
        height: usize,
    ) -> Vec<i32> {
        neighborhood(
            parent,
            x,
            z,
            width,
            height,
            |mut center, n, e, w, s, wx, wz| {
                if center != w || center != n {
                    if w == e && n == s {
                        center = if self.random.at(wx, wz).next(2) == 0 {
                            w
                        } else {
                            n
                        };
                    } else {
                        if w == e {
                            center = w;
                        }
                        if n == s {
                            center = n;
                        }
                    }
                }
                center
            },
        )
    }

    pub(super) fn shore(
        &self,
        parent: &LayerRef,
        x: i32,
        z: i32,
        width: usize,
        height: usize,
    ) -> Vec<i32> {
        neighborhood(parent, x, z, width, height, |center, n, e, w, s, _, _| {
            let adjacent = [n, e, w, s];

            if center == MUSHROOM_ISLAND {
                return if adjacent.contains(&OCEAN) {
                    MUSHROOM_SHORE
                } else {
                    center
                };
            }

            if biome_category(center) == BiomeCategory::Jungle {
                if !adjacent.iter().all(|value| jungle_compatible(*value)) {
                    return JUNGLE_EDGE;
                }
                return if adjacent.iter().any(|value| is_oceanic(*value)) {
                    BEACH
                } else {
                    center
                };
            }

            if matches!(center, EXTREME_HILLS | EXTREME_HILLS_PLUS) {
                return if adjacent.iter().any(|value| is_oceanic(*value)) {
                    STONE_BEACH
                } else {
                    center
                };
            }

            if is_snowy(center) {
                return if adjacent.iter().any(|value| is_oceanic(*value)) {
                    COLD_BEACH
                } else {
                    center
                };
            }

            if matches!(center, MESA | MESA_PLATEAU_F) {
                if adjacent.iter().any(|value| is_oceanic(*value)) {
                    return center;
                }
                return if adjacent.iter().all(|value| is_mesa(*value)) {
                    center
                } else {
                    DESERT
                };
            }

            if !matches!(center, OCEAN | DEEP_OCEAN | RIVER | SWAMPLAND)
                && adjacent.iter().any(|value| is_oceanic(*value))
            {
                BEACH
            } else {
                center
            }
        })
    }

    pub(super) fn river_mixer(
        &self,
        biomes: &LayerRef,
        rivers: &LayerRef,
        x: i32,
        z: i32,
        width: usize,
        height: usize,
    ) -> Vec<i32> {
        let biomes = biomes.area(x, z, width, height);
        let rivers = rivers.area(x, z, width, height);
        biomes
            .into_iter()
            .zip(rivers)
            .map(|(biome, river)| {
                if river != RIVER || is_oceanic(biome) {
                    return biome;
                }
                if biome == ICE_PLAINS {
                    FROZEN_RIVER
                } else if matches!(biome, MUSHROOM_ISLAND | MUSHROOM_SHORE) {
                    MUSHROOM_SHORE
                } else {
                    RIVER
                }
            })
            .collect()
    }
}

fn replace_edge(center: i32, adjacent: &[i32; 4], base: i32, edge: i32) -> Option<i32> {
    if center != base {
        return None;
    }
    Some(
        if adjacent
            .iter()
            .all(|candidate| biome_similar(*candidate, base))
        {
            center
        } else {
            edge
        },
    )
}

fn reduce_river_id(id: i32) -> i32 {
    if id >= 2 { 2 + (id & 1) } else { id }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum BiomeCategory {
    Beach,
    Desert,
    Mountains,
    Forest,
    Snowy,
    Jungle,
    Mesa,
    Mushroom,
    Ocean,
    Plains,
    River,
    Savanna,
    Swamp,
    Taiga,
    StoneBeach,
    Other,
}

fn biome_category(id: i32) -> BiomeCategory {
    match id {
        BEACH | COLD_BEACH => BiomeCategory::Beach,
        DESERT | DESERT_HILLS | 130 => BiomeCategory::Desert,
        EXTREME_HILLS | EXTREME_HILLS_EDGE | EXTREME_HILLS_PLUS | 131 | 162 => {
            BiomeCategory::Mountains
        }
        FOREST | FOREST_HILLS | BIRCH_FOREST | BIRCH_FOREST_HILLS | ROOFED_FOREST | 132 | 155
        | 156 | 157 => BiomeCategory::Forest,
        ICE_PLAINS | ICE_MOUNTAINS | 140 => BiomeCategory::Snowy,
        JUNGLE | JUNGLE_HILLS | JUNGLE_EDGE | 149 | 151 => BiomeCategory::Jungle,
        MESA | MESA_PLATEAU_F | MESA_PLATEAU | 165 | 166 | 167 => BiomeCategory::Mesa,
        MUSHROOM_ISLAND | MUSHROOM_SHORE => BiomeCategory::Mushroom,
        OCEAN | FROZEN_OCEAN | DEEP_OCEAN => BiomeCategory::Ocean,
        PLAINS | 129 => BiomeCategory::Plains,
        RIVER | FROZEN_RIVER => BiomeCategory::River,
        SAVANNA | SAVANNA_PLATEAU | 163 | 164 => BiomeCategory::Savanna,
        SWAMPLAND | 134 => BiomeCategory::Swamp,
        TAIGA | TAIGA_HILLS | COLD_TAIGA | COLD_TAIGA_HILLS | MEGA_TAIGA | MEGA_TAIGA_HILLS
        | 133 | 158 | 160 | 161 => BiomeCategory::Taiga,
        STONE_BEACH => BiomeCategory::StoneBeach,
        _ => BiomeCategory::Other,
    }
}

fn biome_similar(first: i32, second: i32) -> bool {
    if first == second {
        return true;
    }
    if matches!(first, MESA_PLATEAU_F | MESA_PLATEAU) {
        return matches!(second, MESA_PLATEAU_F | MESA_PLATEAU);
    }
    let first_category = biome_category(first);
    first_category != BiomeCategory::Other && first_category == biome_category(second)
}

fn jungle_compatible(id: i32) -> bool {
    matches!(
        biome_category(id),
        BiomeCategory::Jungle | BiomeCategory::Forest | BiomeCategory::Taiga
    ) || is_oceanic(id)
}

fn is_snowy(id: i32) -> bool {
    matches!(
        id,
        FROZEN_OCEAN
            | FROZEN_RIVER
            | ICE_PLAINS
            | ICE_MOUNTAINS
            | COLD_BEACH
            | COLD_TAIGA
            | COLD_TAIGA_HILLS
            | 140
            | 158
    )
}

fn is_mesa(id: i32) -> bool {
    matches!(id, MESA | MESA_PLATEAU_F | MESA_PLATEAU | 165 | 166 | 167)
}

fn mutated_biome(id: i32) -> Option<i32> {
    Some(match id {
        1 => 129,
        2 => 130,
        3 => 131,
        4 => 132,
        5 => 133,
        6 => 134,
        12 => 140,
        21 => 149,
        23 => 151,
        27 => 155,
        28 => 156,
        29 => 157,
        30 => 158,
        32 => 160,
        33 => 161,
        34 => 162,
        35 => 163,
        36 => 164,
        37 => 165,
        38 => 166,
        39 => 167,
        _ => return None,
    })
}
