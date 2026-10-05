use crate::WORLD_HEIGHT;
use crate::population::{PopulationNeighborhood, state};
use crate::population_feature::{
    AIR, BROWN_MUSHROOM, DIRT, FARMLAND, GRASS, ICE, LEAVES, MYCELIUM, PACKED_ICE, PODZOL,
    RED_FLOWER, RED_MUSHROOM, SNOW, SNOW_LAYER, STILL_WATER, TALL_GRASS, YELLOW_FLOWER,
    block_solid_flag, material_is_solid,
};
use crate::population_light::{OverworldFinalizedLighting, finalize_center_lighting};
use crate::population_tick::drain_generation_ticks;
use crate::terrain_shape::noise::{MtRandom, PerlinSimplexNoise};

const FLOWING_WATER: u16 = 8;
const COVERED_BIT: u8 = 8;
const SNOW_NOISE_MIN: f32 = -26.0;
const SNOW_NOISE_MAX: f32 = 26.0;
const SNOW_NOISE_OFFSET: f32 = -0.17;

/// Fixed-target post-decoration mutations that run before final light/height propagation.
#[derive(Debug, Default, Copy, Clone)]
pub struct OverworldPostDecorationFinalizer;

impl OverworldPostDecorationFinalizer {
    pub const fn new() -> Self {
        Self
    }

    pub fn finalize_states(&self, neighborhood: &mut PopulationNeighborhood) {
        fix_water_along_edges(neighborhood);
        apply_generation_seasons(neighborhood);
        neighborhood.with_generation_ticks(drain_generation_ticks);
    }

    pub fn finalize(
        &self,
        neighborhood: &mut PopulationNeighborhood,
    ) -> OverworldFinalizedLighting {
        self.finalize_states(neighborhood);
        finalize_center_lighting(neighborhood)
    }
}

fn fix_water_along_edges(region: &mut PopulationNeighborhood) {
    let center = region.center();
    let origin_x = center.x().wrapping_mul(16);
    let origin_z = center.z().wrapping_mul(16);

    for (local_x, local_z) in edge_offsets() {
        let x = origin_x.wrapping_add(local_x);
        let z = origin_z.wrapping_add(local_z);
        let height = region.generation_height(x, z);
        let mut was_air = true;

        for y in 0..height {
            if region.block_id(x, y, z) == STILL_WATER {
                if was_air {
                    let _ = region.set_state(x, y, z, state(FLOWING_WATER, 0));
                    region.with_generation_ticks(|_, queue| {
                        queue.add(x, y, z, FLOWING_WATER, 1);
                    });
                    was_air = false;
                }
            } else {
                was_air = true;
            }
        }
    }
}

fn edge_offsets() -> impl Iterator<Item = (i32, i32)> {
    let west = (0..16).map(|z| (0, z));
    let north_south = (1..15).flat_map(|x| [(x, 0), (x, 15)]);
    let east = (0..16).map(|z| (15, z));
    west.chain(north_south).chain(east)
}

fn apply_generation_seasons(region: &mut PopulationNeighborhood) {
    if !center_has_dirty_snow_biome(region) {
        return;
    }

    let mut random = MtRandom::new(89_328);
    let noise = PerlinSimplexNoise::new(&mut random, 5);
    let center = region.center();
    let origin_x = center.x().wrapping_mul(16);
    let origin_z = center.z().wrapping_mul(16);

    for local_z in 0..16_i32 {
        for local_x in 0..16_i32 {
            let x = origin_x.wrapping_add(local_x);
            let z = origin_z.wrapping_add(local_z);
            let Some(profile) = region.biome_id(x, z).and_then(snow_profile) else {
                continue;
            };

            let top = top_rain_pos(region, x, z);
            if !can_place_generation_top_snow(region, top, profile) {
                continue;
            }

            let value = noise.value_3d(top.0 as f32, top.1 as f32, top.2 as f32);
            let normalized =
                (value - SNOW_NOISE_MIN) / (SNOW_NOISE_MAX - SNOW_NOISE_MIN) + SNOW_NOISE_OFFSET;
            let mut depth = (normalized * f32::from(profile.min_layers)).ceil() as i32;

            for (nx, nz) in [
                (x, z.wrapping_sub(1)),
                (x, z.wrapping_add(1)),
                (x.wrapping_sub(1), z),
                (x.wrapping_add(1), z),
            ] {
                let neighbor_y = top_rain_pos(region, nx, nz).1;
                if neighbor_y > top.1 {
                    depth += 1;
                } else if neighbor_y < top.1 && depth > 1 {
                    depth -= 1;
                }
            }

            depth = depth.clamp(0, i32::from(profile.max_layers));
            rebuild_top_snow_to_depth(region, top, depth);
        }
    }
}

fn center_has_dirty_snow_biome(region: &PopulationNeighborhood) -> bool {
    region
        .center_biome_ids()
        .iter()
        .copied()
        .any(|id| snow_profile(id).is_some_and(|profile| profile.current_layers > 0))
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct SnowProfile {
    current_layers: u8,
    min_layers: u8,
    max_layers: u8,
}

const fn snow_profile(id: u8) -> Option<SnowProfile> {
    let profile = match id {
        10 | 11 | 26 => SnowProfile {
            current_layers: 1,
            min_layers: 1,
            max_layers: 2,
        },
        12 => SnowProfile {
            current_layers: 2,
            min_layers: 2,
            max_layers: 8,
        },
        13 => SnowProfile {
            current_layers: 3,
            min_layers: 3,
            max_layers: 12,
        },
        30 | 31 => SnowProfile {
            current_layers: 1,
            min_layers: 1,
            max_layers: 4,
        },
        140 => SnowProfile {
            current_layers: 4,
            min_layers: 4,
            max_layers: 12,
        },
        // Generic MutatedBiome copies mSnowAccumulation from Cold Taiga but does not copy
        // mMinSnowLevel/mMaxSnowLevel. Its base Biome constructor therefore leaves min=0,max=1.
        158 => SnowProfile {
            current_layers: 1,
            min_layers: 0,
            max_layers: 1,
        },
        _ => return None,
    };
    Some(profile)
}

fn top_rain_pos(region: &PopulationNeighborhood, x: i32, z: i32) -> (i32, i32, i32) {
    let mut y = WORLD_HEIGHT as i32;
    while y > 0 {
        let id = region.block_id(x, y, z);
        if material_blocks_motion_or_is_liquid(id) {
            return (x, y.wrapping_add(1).min(WORLD_HEIGHT as i32), z);
        }
        y -= 1;
    }
    (x, -1, z)
}

fn can_place_generation_top_snow(
    region: &PopulationNeighborhood,
    pos: (i32, i32, i32),
    _profile: SnowProfile,
) -> bool {
    let (x, y, z) = pos;
    if !(0..WORLD_HEIGHT as i32).contains(&y) {
        return false;
    }

    let current = region.state(x, y, z).unwrap_or(state(AIR, 0));
    let current_id = current >> 4;
    if current_id == SNOW_LAYER {
        return true;
    }
    if current_id != AIR && !recoverable_top_snow_block(current_id) {
        return false;
    }

    let below = region
        .state(x, y.wrapping_sub(1), z)
        .unwrap_or(state(AIR, 0));
    let below_id = below >> 4;
    let below_data = (below & 0x0f) as u8;

    if below_id == SNOW_LAYER {
        return top_snow_height(below_data) == 8;
    }
    if matches!(below_id, ICE | PACKED_ICE) {
        return false;
    }

    if !block_solid_flag(below_id)
        && !matches!(
            below_id,
            LEAVES | 161 | GRASS | DIRT | FARMLAND | MYCELIUM | PODZOL | 198
        )
    {
        return false;
    }

    material_blocks_motion(below_id)
}

fn rebuild_top_snow_to_depth(
    region: &mut PopulationNeighborhood,
    pos: (i32, i32, i32),
    mut depth: i32,
) {
    let (x, start_y, z) = pos;
    let mut top_y = start_y;
    while top_y < WORLD_HEIGHT as i32 && region.block_id(x, top_y, z) != AIR {
        top_y += 1;
    }

    let mut bottom_y = top_y.wrapping_sub(1);
    let mut full_snow_blocks = 0_i32;
    while bottom_y >= 0 {
        let id = region.block_id(x, bottom_y, z);
        if id == SNOW {
            full_snow_blocks += 1;
        } else if id != SNOW_LAYER {
            break;
        }
        bottom_y -= 1;
    }

    if bottom_y >= 0 && !recoverable_top_snow_block(region.block_id(x, bottom_y, z)) {
        bottom_y += 1;
    }

    while full_snow_blocks > 0 && bottom_y < WORLD_HEIGHT as i32 {
        let _ = region.set_state(x, bottom_y, z, state(SNOW, 0));
        let _ = region.set_extra_data(x, bottom_y, z, 0);
        bottom_y += 1;
        full_snow_blocks -= 1;
    }

    while depth > 0 && bottom_y < WORLD_HEIGHT as i32 {
        let set_depth = depth.min(8) as u8;
        let data = top_snow_data(region, x, bottom_y, z, set_depth);
        let _ = region.set_state(x, bottom_y, z, state(SNOW_LAYER, data));
        depth -= i32::from(set_depth);
        bottom_y += 1;
    }

    while bottom_y < top_y && bottom_y < WORLD_HEIGHT as i32 {
        let _ = region.set_state(x, bottom_y, z, state(AIR, 0));
        let _ = region.set_extra_data(x, bottom_y, z, 0);
        bottom_y += 1;
    }
}

fn top_snow_data(region: &mut PopulationNeighborhood, x: i32, y: i32, z: i32, height: u8) -> u8 {
    let current = region.state(x, y, z).unwrap_or(state(AIR, 0));
    let id = current >> 4;
    let old_data = (current & 0x0f) as u8;

    if id == SNOW_LAYER {
        return (old_data & COVERED_BIT) | (height.saturating_sub(1) & 7);
    }

    let mut data = height.saturating_sub(1) & 7;
    if id != AIR && recoverable_top_snow_block(id) {
        let extra = (u16::from(old_data) << 8) | (id & 0xff);
        let _ = region.set_extra_data(x, y, z, extra);
        data |= COVERED_BIT;
    }
    data
}

const fn top_snow_height(data: u8) -> u8 {
    (data & 7) + 1
}

const fn recoverable_top_snow_block(id: u16) -> bool {
    matches!(
        id,
        TALL_GRASS | YELLOW_FLOWER | RED_FLOWER | BROWN_MUSHROOM | RED_MUSHROOM
    )
}

const fn material_blocks_motion(id: u16) -> bool {
    material_is_solid(id)
        || matches!(
            id,
            LEAVES | 161 | FARMLAND | 63 | 64 | 65 | 68 | 71 | 83 | 90 | 119 | 193..=197
        )
}

const fn material_blocks_motion_or_is_liquid(id: u16) -> bool {
    material_blocks_motion(id) || matches!(id, 8..=11)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChunkCoord, population::PopulationNeighborhood};

    fn block_count(region: &PopulationNeighborhood, id: u16) -> usize {
        region
            .state_planes()
            .flat_map(|states| states.iter())
            .filter(|value| (**value >> 4) == id)
            .count()
    }

    fn hash_center_states(region: &PopulationNeighborhood) -> u64 {
        region
            .center_states()
            .iter()
            .fold(0xcbf2_9ce4_8422_2325_u64, |mut hash, value| {
                for byte in value.to_le_bytes() {
                    hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
                }
                hash
            })
    }

    #[test]
    fn exact_edge_offset_order_contains_sixty_columns() {
        let offsets: Vec<_> = edge_offsets().collect();
        assert_eq!(offsets.len(), 60);
        assert_eq!(offsets[0], (0, 0));
        assert_eq!(offsets[15], (0, 15));
        assert_eq!(offsets[16], (1, 0));
        assert_eq!(offsets[43], (14, 15));
        assert_eq!(offsets[44], (15, 0));
        assert_eq!(offsets[59], (15, 15));
    }

    #[test]
    fn edge_fix_converts_only_first_still_water_in_each_vertical_run() {
        let center = ChunkCoord::new(0, 0);
        let mut region = PopulationNeighborhood::filled(center, state(AIR, 0), 1);
        region.set_generation_height_for_test(0, 0, 8);
        for y in 1..=3 {
            let _ = region.set_state(0, y, 0, state(STILL_WATER, 0));
        }
        for y in 5..=6 {
            let _ = region.set_state(0, y, 0, state(STILL_WATER, 0));
        }

        fix_water_along_edges(&mut region);

        assert_eq!(region.block_id(0, 1, 0), FLOWING_WATER);
        assert_eq!(region.block_id(0, 2, 0), STILL_WATER);
        assert_eq!(region.block_id(0, 3, 0), STILL_WATER);
        assert_eq!(region.block_id(0, 5, 0), FLOWING_WATER);
        assert_eq!(region.block_id(0, 6, 0), STILL_WATER);
    }

    #[test]
    fn independent_cpp_edge_water_fixture_matches() {
        // Standalone C++ oracle translated from the target edge-offset scan. It hashes the
        // center chunk immediately after the two still-water runs are converted at their tops.
        let center = ChunkCoord::new(0, 0);
        let mut region = PopulationNeighborhood::filled(center, state(AIR, 0), 1);
        region.set_generation_height_for_test(0, 0, 8);
        for y in 1..=3 {
            let _ = region.set_state(0, y, 0, state(STILL_WATER, 0));
        }
        for y in 5..=6 {
            let _ = region.set_state(0, y, 0, state(STILL_WATER, 0));
        }

        fix_water_along_edges(&mut region);

        assert_eq!(hash_center_states(&region), 0x7aca_e6ad_6447_a3b5);
        assert_eq!(region.block_id(0, 1, 0), FLOWING_WATER);
        assert_eq!(region.block_id(0, 2, 0), STILL_WATER);
        assert_eq!(region.block_id(0, 5, 0), FLOWING_WATER);
    }

    #[test]
    fn snow_profiles_preserve_fixed_target_accumulation_quirks() {
        assert_eq!(
            snow_profile(12),
            Some(SnowProfile {
                current_layers: 2,
                min_layers: 2,
                max_layers: 8,
            })
        );
        assert_eq!(
            snow_profile(140),
            Some(SnowProfile {
                current_layers: 4,
                min_layers: 4,
                max_layers: 12,
            })
        );
        assert_eq!(
            snow_profile(158),
            Some(SnowProfile {
                current_layers: 1,
                min_layers: 0,
                max_layers: 1,
            })
        );
        assert_eq!(snow_profile(1), None);
    }

    #[test]
    fn covered_top_snow_preserves_recoverable_block_extra_data() {
        let center = ChunkCoord::new(0, 0);
        let mut region = PopulationNeighborhood::filled(center, state(AIR, 0), 12);
        let _ = region.set_state(8, 63, 8, state(GRASS, 0));
        let _ = region.set_state(8, 64, 8, state(RED_FLOWER, 5));

        rebuild_top_snow_to_depth(&mut region, (8, 64, 8), 2);

        assert_eq!(
            region.state(8, 64, 8),
            Some(state(SNOW_LAYER, COVERED_BIT | 1))
        );
        assert_eq!(region.extra_data(8, 64, 8), (5_u16 << 8) | RED_FLOWER);
    }

    #[test]
    fn independent_cpp_covered_top_snow_fixture_matches() {
        // Standalone C++ oracle translated from TopSnowBlock recovery + rebuild semantics.
        // Ten layers cover a data-5 red flower with a full 8-layer top-snow state plus a
        // second 2-layer state, preserving the flower in extra data.
        let center = ChunkCoord::new(0, 0);
        let mut region = PopulationNeighborhood::filled(center, state(AIR, 0), 12);
        let _ = region.set_state(8, 63, 8, state(GRASS, 0));
        let _ = region.set_state(8, 64, 8, state(RED_FLOWER, 5));

        rebuild_top_snow_to_depth(&mut region, (8, 64, 8), 10);

        let mut hash = hash_center_states(&region);
        let key = ((64_usize << 8) | (8 << 4) | 8) as u16;
        let extra = region.extra_data(8, 64, 8);
        for byte in key.to_le_bytes().into_iter().chain(extra.to_le_bytes()) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        }

        assert_eq!(hash, 0xd764_a759_a386_f65e);
        assert_eq!(region.state(8, 64, 8), Some(state(SNOW_LAYER, 15)));
        assert_eq!(region.state(8, 65, 8), Some(state(SNOW_LAYER, 1)));
        assert_eq!(extra, (5_u16 << 8) | RED_FLOWER);
    }

    #[test]
    fn seasons_places_snow_only_for_target_snow_biomes() {
        let center = ChunkCoord::new(0, 0);
        let mut cold = PopulationNeighborhood::filled(center, state(AIR, 0), 12);
        for z in -16..32 {
            for x in -16..32 {
                let _ = cold.set_state(x, 63, z, state(GRASS, 0));
            }
        }
        apply_generation_seasons(&mut cold);
        assert!(block_count(&cold, SNOW_LAYER) > 0);

        let mut warm = PopulationNeighborhood::filled(center, state(AIR, 0), 1);
        for z in -16..32 {
            for x in -16..32 {
                let _ = warm.set_state(x, 63, z, state(GRASS, 0));
            }
        }
        apply_generation_seasons(&mut warm);
        assert_eq!(block_count(&warm, SNOW_LAYER), 0);
    }
}
