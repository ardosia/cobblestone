use std::collections::VecDeque;

use crate::population::PopulationNeighborhood;
use crate::{CHUNK_COLUMN_COUNT, CHUNK_NIBBLE_BYTES, WORLD_HEIGHT};

const NEIGHBORHOOD_EDGE: usize = 48;
const NEIGHBORHOOD_AREA: usize = NEIGHBORHOOD_EDGE * NEIGHBORHOOD_EDGE;
const NEIGHBORHOOD_VOLUME: usize = NEIGHBORHOOD_AREA * WORLD_HEIGHT;
const CENTER_OFFSET: i32 = 16;

/// Final fixed-target light/height payload for the post-processed center chunk.
///
/// The 0.15.10 target starts generated chunks with height-derived skylight, defers block emitters
/// during generation, then runs the final light update after population finalizers. Recomputing
/// the stable light field over the supplied 3x3 neighborhood gives the center chunk the same
/// bounded (15-block) light influence while keeping runtime lighting outside worldgen.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct OverworldFinalizedLighting {
    height_map: Vec<u8>,
    sky_light: Vec<u8>,
    block_light: Vec<u8>,
}

impl OverworldFinalizedLighting {
    pub fn height_map(&self) -> &[u8] {
        &self.height_map
    }

    pub fn sky_light(&self) -> &[u8] {
        &self.sky_light
    }

    pub fn block_light(&self) -> &[u8] {
        &self.block_light
    }
}

pub(crate) fn finalize_center_lighting(
    region: &PopulationNeighborhood,
) -> OverworldFinalizedLighting {
    let heights = neighborhood_height_map(region);
    let sky = propagate_sky_light(region, &heights);
    let block = propagate_block_light(region);

    OverworldFinalizedLighting {
        height_map: center_height_map(&heights),
        sky_light: pack_center_light(&sky),
        block_light: pack_center_light(&block),
    }
}

fn neighborhood_height_map(region: &PopulationNeighborhood) -> Vec<u8> {
    let mut heights = vec![0_u8; NEIGHBORHOOD_AREA];
    let center = region.center();
    let origin_x = center.x().wrapping_mul(16);
    let origin_z = center.z().wrapping_mul(16);

    for local_z in 0..NEIGHBORHOOD_EDGE {
        for local_x in 0..NEIGHBORHOOD_EDGE {
            let world_x = origin_x.wrapping_add(local_x as i32 - CENTER_OFFSET);
            let world_z = origin_z.wrapping_add(local_z as i32 - CENTER_OFFSET);
            let mut y = WORLD_HEIGHT;
            while y > 0 && target_light_block(region.block_id(world_x, y as i32 - 1, world_z)) == 0
            {
                y -= 1;
            }
            heights[local_x + local_z * NEIGHBORHOOD_EDGE] = y as u8;
        }
    }

    heights
}

fn propagate_sky_light(region: &PopulationNeighborhood, heights: &[u8]) -> Vec<u8> {
    let mut light = vec![0_u8; NEIGHBORHOOD_VOLUME];
    let mut queue = VecDeque::new();

    for z in 0..NEIGHBORHOOD_EDGE {
        for x in 0..NEIGHBORHOOD_EDGE {
            let height = usize::from(heights[x + z * NEIGHBORHOOD_EDGE]);
            for y in height..WORLD_HEIGHT {
                let index = neighborhood_block_index(x, y, z);
                light[index] = 15;
                queue.push_back((x, y, z));
            }
        }
    }

    propagate(region, &mut light, queue);
    light
}

fn propagate_block_light(region: &PopulationNeighborhood) -> Vec<u8> {
    let mut light = vec![0_u8; NEIGHBORHOOD_VOLUME];
    let mut queue = VecDeque::new();
    let center = region.center();
    let origin_x = center.x().wrapping_mul(16);
    let origin_z = center.z().wrapping_mul(16);

    for y in 0..WORLD_HEIGHT {
        for z in 0..NEIGHBORHOOD_EDGE {
            let world_z = origin_z.wrapping_add(z as i32 - CENTER_OFFSET);
            for x in 0..NEIGHBORHOOD_EDGE {
                let world_x = origin_x.wrapping_add(x as i32 - CENTER_OFFSET);
                let emission = target_light_emission(region.block_id(world_x, y as i32, world_z));
                if emission == 0 {
                    continue;
                }
                let index = neighborhood_block_index(x, y, z);
                light[index] = emission;
                queue.push_back((x, y, z));
            }
        }
    }

    propagate(region, &mut light, queue);
    light
}

fn propagate(
    region: &PopulationNeighborhood,
    light: &mut [u8],
    mut queue: VecDeque<(usize, usize, usize)>,
) {
    let center = region.center();
    let origin_x = center.x().wrapping_mul(16);
    let origin_z = center.z().wrapping_mul(16);

    while let Some((x, y, z)) = queue.pop_front() {
        let source = light[neighborhood_block_index(x, y, z)];
        if source <= 1 {
            continue;
        }

        for (nx, ny, nz) in light_neighbors(x, y, z) {
            let world_x = origin_x.wrapping_add(nx as i32 - CENTER_OFFSET);
            let world_z = origin_z.wrapping_add(nz as i32 - CENTER_OFFSET);
            let attenuation =
                target_light_block(region.block_id(world_x, ny as i32, world_z)).max(1);
            let candidate = source.saturating_sub(attenuation);
            if candidate == 0 {
                continue;
            }

            let index = neighborhood_block_index(nx, ny, nz);
            if candidate > light[index] {
                light[index] = candidate;
                queue.push_back((nx, ny, nz));
            }
        }
    }
}

fn light_neighbors(x: usize, y: usize, z: usize) -> impl Iterator<Item = (usize, usize, usize)> {
    let mut values = [(0_usize, 0_usize, 0_usize); 6];
    let mut len = 0;

    if x > 0 {
        values[len] = (x - 1, y, z);
        len += 1;
    }
    if x + 1 < NEIGHBORHOOD_EDGE {
        values[len] = (x + 1, y, z);
        len += 1;
    }
    if y > 0 {
        values[len] = (x, y - 1, z);
        len += 1;
    }
    if y + 1 < WORLD_HEIGHT {
        values[len] = (x, y + 1, z);
        len += 1;
    }
    if z > 0 {
        values[len] = (x, y, z - 1);
        len += 1;
    }
    if z + 1 < NEIGHBORHOOD_EDGE {
        values[len] = (x, y, z + 1);
        len += 1;
    }

    values.into_iter().take(len)
}

fn center_height_map(heights: &[u8]) -> Vec<u8> {
    let mut center = vec![0_u8; CHUNK_COLUMN_COUNT];
    for z in 0..16 {
        for x in 0..16 {
            center[x + z * 16] = heights[(x + 16) + (z + 16) * NEIGHBORHOOD_EDGE];
        }
    }
    center
}

fn pack_center_light(light: &[u8]) -> Vec<u8> {
    let mut packed = vec![0_u8; CHUNK_NIBBLE_BYTES];
    for y in 0..WORLD_HEIGHT {
        for z in 0..16 {
            for x in 0..16 {
                let source = neighborhood_block_index(x + 16, y, z + 16);
                let target = (y << 8) | (z << 4) | x;
                write_nibble(&mut packed, target, light[source]);
            }
        }
    }
    packed
}

fn write_nibble(bytes: &mut [u8], index: usize, value: u8) {
    debug_assert!(value <= 15);
    let byte = &mut bytes[index >> 1];
    if index & 1 == 0 {
        *byte = (*byte & 0xf0) | value;
    } else {
        *byte = (*byte & 0x0f) | (value << 4);
    }
}

#[cfg(test)]
fn read_nibble(bytes: &[u8], index: usize) -> u8 {
    let value = bytes[index >> 1];
    if index & 1 == 0 {
        value & 0x0f
    } else {
        value >> 4
    }
}

const fn neighborhood_block_index(x: usize, y: usize, z: usize) -> usize {
    y * NEIGHBORHOOD_AREA + z * NEIGHBORHOOD_EDGE + x
}

/// Exact Block::mLightBlock values for the fixed-target block states worldgen can emit.
///
/// Block::setSolid(false) rewrites the table back to zero, which is why several visually solid
/// partial blocks (stairs, slabs, fences, chests, doors, path, portal frame) are zero here.
const fn target_light_block(id: u16) -> u8 {
    match id {
        8 | 9 | 79 => 3,
        18 | 30 | 161 => 1,
        0 | 6 | 20 | 31 | 32 | 37 | 38 | 39 | 40 | 44 | 50 | 51 | 52 | 53 | 54 | 55 | 59 | 60
        | 64 | 65 | 66 | 67 | 69 | 70 | 71 | 78 | 81 | 83 | 85 | 93 | 101 | 102 | 106 | 109
        | 111 | 119 | 120 | 127 | 128 | 131 | 132 | 134 | 140 | 141 | 142 | 143 | 163 | 175
        | 193 | 196 | 198 | 244 => 0,
        _ => 15,
    }
}

/// Exact Block::mLightEmission values needed by fixed-target Infinite generation.
const fn target_light_emission(id: u16) -> u8 {
    match id {
        10 | 11 | 51 | 119 => 15,
        50 => 14,
        52 => 3,
        39 => 1,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::population::state;
    use crate::{CHUNK_BLOCK_COUNT, ChunkCoord};

    fn light_at(bytes: &[u8], x: usize, y: usize, z: usize) -> u8 {
        read_nibble(bytes, (y << 8) | (z << 4) | x)
    }

    #[test]
    fn target_generated_light_properties_cover_fixed_target_quirks() {
        for (id, expected) in [
            (0, 0),
            (8, 3),
            (9, 3),
            (10, 15),
            (11, 15),
            (18, 1),
            (30, 1),
            (44, 0),
            (50, 0),
            (52, 0),
            (53, 0),
            (54, 0),
            (60, 0),
            (79, 3),
            (85, 0),
            (119, 0),
            (120, 0),
            (161, 1),
            (174, 15),
            (198, 0),
        ] {
            assert_eq!(target_light_block(id), expected, "id={id}");
        }

        for (id, expected) in [
            (10, 15),
            (11, 15),
            (39, 1),
            (50, 14),
            (51, 15),
            (52, 3),
            (119, 15),
        ] {
            assert_eq!(target_light_emission(id), expected, "id={id}");
        }
    }

    #[test]
    fn final_height_uses_light_block_not_non_air_height() {
        let mut region = PopulationNeighborhood::filled(ChunkCoord::new(0, 0), state(0, 0), 1);
        let _ = region.set_state(8, 40, 8, state(1, 0));
        let _ = region.set_state(8, 60, 8, state(20, 0));
        let _ = region.set_state(8, 80, 8, state(50, 0));

        let result = finalize_center_lighting(&region);
        assert_eq!(result.height_map()[8 + 8 * 16], 41);
    }

    #[test]
    fn final_sky_light_attenuates_through_target_water_opacity() {
        let mut region = PopulationNeighborhood::filled(ChunkCoord::new(0, 0), state(0, 0), 1);
        for z in -16..32 {
            for x in -16..32 {
                let _ = region.set_state(x, 62, z, state(1, 0));
            }
        }
        let _ = region.set_state(8, 63, 8, state(9, 0));

        let result = finalize_center_lighting(&region);
        assert_eq!(result.height_map()[8 + 8 * 16], 64);
        assert_eq!(light_at(result.sky_light(), 8, 64, 8), 15);
        assert_eq!(light_at(result.sky_light(), 8, 63, 8), 12);
        assert_eq!(light_at(result.sky_light(), 8, 62, 8), 0);
    }

    #[test]
    fn final_block_light_propagates_target_emission_and_respects_opaque_blocks() {
        let mut region = PopulationNeighborhood::filled(ChunkCoord::new(0, 0), state(1, 0), 1);
        for x in 6..=11 {
            let _ = region.set_state(x, 64, 8, state(0, 0));
        }
        let _ = region.set_state(8, 64, 8, state(50, 0));
        let _ = region.set_state(10, 64, 8, state(1, 0));

        let result = finalize_center_lighting(&region);
        assert_eq!(light_at(result.block_light(), 8, 64, 8), 14);
        assert_eq!(light_at(result.block_light(), 9, 64, 8), 13);
        assert_eq!(light_at(result.block_light(), 10, 64, 8), 0);
        assert_eq!(light_at(result.block_light(), 11, 64, 8), 0);
    }

    #[test]
    fn neighbor_emitter_can_contribute_across_center_chunk_boundary() {
        let mut region = PopulationNeighborhood::filled(ChunkCoord::new(0, 0), state(1, 0), 1);
        for x in 14..=17 {
            let _ = region.set_state(x, 64, 8, state(0, 0));
        }
        let _ = region.set_state(16, 64, 8, state(50, 0));

        let result = finalize_center_lighting(&region);
        assert_eq!(light_at(result.block_light(), 15, 64, 8), 13);
        assert_eq!(light_at(result.block_light(), 14, 64, 8), 12);
    }

    #[test]
    fn independent_cpp_final_light_fixture_matches() {
        // Standalone C++ oracle over a synthetic 48x48 target neighborhood. It independently
        // applies target mLightBlock/mLightEmission values, final height recomputation, stable
        // sky/block propagation, nibble packing, water attenuation, and an east-neighbor torch.
        let mut region = PopulationNeighborhood::filled(ChunkCoord::new(0, 0), state(0, 0), 1);
        for z in -16..32 {
            for x in -16..32 {
                let _ = region.set_state(x, 62, z, state(1, 0));
            }
        }
        let _ = region.set_state(8, 63, 8, state(9, 0));
        let _ = region.set_state(16, 64, 8, state(50, 0));

        let result = finalize_center_lighting(&region);
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in result
            .height_map()
            .iter()
            .chain(result.sky_light())
            .chain(result.block_light())
        {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3);
        }

        assert_eq!(hash, 0xeb4c_ef1c_923e_ce33);
        assert_eq!(result.height_map()[8 + 8 * 16], 64);
        assert_eq!(light_at(result.sky_light(), 8, 63, 8), 12);
        assert_eq!(light_at(result.block_light(), 15, 64, 8), 13);
        assert_eq!(light_at(result.block_light(), 14, 64, 8), 12);
    }

    #[test]
    fn packed_light_planes_match_chunk_storage_shape() {
        let region = PopulationNeighborhood::filled(ChunkCoord::new(0, 0), state(0, 0), 1);
        let result = finalize_center_lighting(&region);
        assert_eq!(result.height_map().len(), CHUNK_COLUMN_COUNT);
        assert_eq!(result.sky_light().len(), CHUNK_NIBBLE_BYTES);
        assert_eq!(result.block_light().len(), CHUNK_NIBBLE_BYTES);
        assert_eq!(CHUNK_BLOCK_COUNT / 2, result.sky_light().len());
    }
}
