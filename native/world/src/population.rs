use std::array;
use std::collections::BTreeMap;

use crate::population_tick::GenerationTickQueue;
use crate::terrain_shape::noise::MtRandom;
use crate::{CHUNK_BLOCK_COUNT, CHUNK_COLUMN_COUNT, ChunkCoord, OverworldCaveCarver, WORLD_HEIGHT};

#[derive(Debug, Clone, Eq, PartialEq)]
struct PopulationChunk {
    states: Vec<u16>,
    biome_ids: Vec<u8>,
    generation_height_map: Vec<u8>,
    extra_data: BTreeMap<u16, u16>,
}

#[derive(Debug, Clone)]
pub struct PopulationNeighborhood {
    center: ChunkCoord,
    chunks: [PopulationChunk; 9],
    generation_ticks: Option<GenerationTickQueue>,
}

impl PartialEq for PopulationNeighborhood {
    fn eq(&self, other: &Self) -> bool {
        self.center == other.center && self.chunks == other.chunks
    }
}

impl Eq for PopulationNeighborhood {}

impl PopulationNeighborhood {
    pub fn from_carved(seed: i32, center: ChunkCoord) -> Self {
        let carver = OverworldCaveCarver::new(seed);
        let chunks = array::from_fn(|index| {
            let offset_x = (index % 3) as i32 - 1;
            let offset_z = (index / 3) as i32 - 1;
            let position = ChunkCoord::new(
                center.x().wrapping_add(offset_x),
                center.z().wrapping_add(offset_z),
            );
            let (states, biome_ids) = carver.generate(position).into_parts();
            debug_assert_eq!(states.len(), CHUNK_BLOCK_COUNT);
            debug_assert_eq!(biome_ids.len(), CHUNK_COLUMN_COUNT);
            let generation_height_map = generation_height_map(&states);
            PopulationChunk {
                states,
                biome_ids,
                generation_height_map,
                extra_data: BTreeMap::new(),
            }
        });

        Self {
            center,
            chunks,
            generation_ticks: Some(GenerationTickQueue::new_target_seeded()),
        }
    }

    pub fn center(&self) -> ChunkCoord {
        self.center
    }

    pub fn center_states(&self) -> &[u16] {
        &self.chunks[4].states
    }

    pub fn center_biome_ids(&self) -> &[u8] {
        &self.chunks[4].biome_ids
    }

    pub fn chunk_states(&self, offset_x: i32, offset_z: i32) -> Option<&[u16]> {
        neighborhood_index(offset_x, offset_z).map(|index| self.chunks[index].states.as_slice())
    }

    pub fn chunk_biome_ids(&self, offset_x: i32, offset_z: i32) -> Option<&[u8]> {
        neighborhood_index(offset_x, offset_z).map(|index| self.chunks[index].biome_ids.as_slice())
    }

    pub(crate) fn biome_id(&self, world_x: i32, world_z: i32) -> Option<u8> {
        let (chunk_index, local_x, local_z) = self.resolve(world_x, world_z)?;
        Some(self.chunks[chunk_index].biome_ids[local_x + local_z * 16])
    }

    pub(crate) fn block_id(&self, world_x: i32, y: i32, world_z: i32) -> u16 {
        self.state(world_x, y, world_z)
            .map(|value| value >> 4)
            .unwrap_or(0)
    }

    /// Target LevelChunk heightmap captured immediately after cave generation.
    ///
    /// Fixed-target post-process block writes do not recalculate this map, so lakes, structures,
    /// dungeons, freeze, and earlier decorator writes intentionally leave later feature heightmap
    /// queries unchanged.
    pub(crate) fn generation_height(&self, world_x: i32, world_z: i32) -> i32 {
        let Some((chunk_index, local_x, local_z)) = self.resolve(world_x, world_z) else {
            return 0;
        };
        i32::from(self.chunks[chunk_index].generation_height_map[local_x + local_z * 16])
    }

    pub(crate) fn state(&self, world_x: i32, y: i32, world_z: i32) -> Option<u16> {
        if !(0..WORLD_HEIGHT as i32).contains(&y) {
            return None;
        }
        let (chunk_index, local_x, local_z) = self.resolve(world_x, world_z)?;
        Some(self.chunks[chunk_index].states[block_index(local_x, y as usize, local_z)])
    }

    pub(crate) fn set_state(&mut self, world_x: i32, y: i32, world_z: i32, value: u16) -> bool {
        if !(0..WORLD_HEIGHT as i32).contains(&y) {
            return false;
        }
        let Some((chunk_index, local_x, local_z)) = self.resolve(world_x, world_z) else {
            return false;
        };
        self.chunks[chunk_index].states[block_index(local_x, y as usize, local_z)] = value;
        true
    }

    #[cfg(test)]
    pub(crate) fn extra_data(&self, world_x: i32, y: i32, world_z: i32) -> u16 {
        if !(0..WORLD_HEIGHT as i32).contains(&y) {
            return 0;
        }
        let Some((chunk_index, local_x, local_z)) = self.resolve(world_x, world_z) else {
            return 0;
        };
        self.chunks[chunk_index]
            .extra_data
            .get(&(block_index(local_x, y as usize, local_z) as u16))
            .copied()
            .unwrap_or(0)
    }

    pub(crate) fn set_extra_data(
        &mut self,
        world_x: i32,
        y: i32,
        world_z: i32,
        value: u16,
    ) -> bool {
        if !(0..WORLD_HEIGHT as i32).contains(&y) {
            return false;
        }
        let Some((chunk_index, local_x, local_z)) = self.resolve(world_x, world_z) else {
            return false;
        };
        let key = block_index(local_x, y as usize, local_z) as u16;
        if value == 0 {
            self.chunks[chunk_index].extra_data.remove(&key);
        } else {
            self.chunks[chunk_index].extra_data.insert(key, value);
        }
        true
    }

    pub(crate) fn with_generation_ticks<R>(
        &mut self,
        f: impl FnOnce(&mut Self, &mut GenerationTickQueue) -> R,
    ) -> R {
        let mut queue = self
            .generation_ticks
            .take()
            .expect("generation tick queue is not reentrant");
        let result = f(self, &mut queue);
        debug_assert!(self.generation_ticks.is_none());
        self.generation_ticks = Some(queue);
        result
    }

    pub(crate) fn above_top_solid_block(
        &self,
        world_x: i32,
        world_z: i32,
        include_water: bool,
    ) -> i32 {
        for y in (0..WORLD_HEIGHT as i32).rev() {
            let id = self.block_id(world_x, y, world_z);
            if top_solid_for_population(id, include_water) {
                return y + 1;
            }
        }
        0
    }

    #[cfg(test)]
    pub(crate) fn state_planes(&self) -> impl Iterator<Item = &[u16]> {
        self.chunks.iter().map(|chunk| chunk.states.as_slice())
    }

    #[cfg(test)]
    pub(crate) fn set_generation_height_for_test(
        &mut self,
        world_x: i32,
        world_z: i32,
        height: u8,
    ) {
        let (chunk_index, local_x, local_z) = self
            .resolve(world_x, world_z)
            .expect("test height coordinate must be inside population neighborhood");
        self.chunks[chunk_index].generation_height_map[local_x + local_z * 16] = height;
    }

    fn resolve(&self, world_x: i32, world_z: i32) -> Option<(usize, usize, usize)> {
        let origin_x = self.center.x().wrapping_mul(16);
        let origin_z = self.center.z().wrapping_mul(16);
        let relative_x = world_x.wrapping_sub(origin_x);
        let relative_z = world_z.wrapping_sub(origin_z);
        if !(-16..48).contains(&relative_x) || !(-16..48).contains(&relative_z) {
            return None;
        }

        let offset_x = relative_x.div_euclid(16);
        let offset_z = relative_z.div_euclid(16);
        let local_x = relative_x.rem_euclid(16) as usize;
        let local_z = relative_z.rem_euclid(16) as usize;
        neighborhood_index(offset_x, offset_z).map(|index| (index, local_x, local_z))
    }

    #[cfg(test)]
    pub(crate) fn filled(center: ChunkCoord, state: u16, biome_id: u8) -> Self {
        Self {
            center,
            chunks: array::from_fn(|_| PopulationChunk {
                states: vec![state; CHUNK_BLOCK_COUNT],
                biome_ids: vec![biome_id; CHUNK_COLUMN_COUNT],
                generation_height_map: vec![
                    if generation_height_blocking(state >> 4) {
                        WORLD_HEIGHT as u8
                    } else {
                        0
                    };
                    CHUNK_COLUMN_COUNT
                ],
                extra_data: BTreeMap::new(),
            }),
            generation_ticks: Some(GenerationTickQueue::with_seed(0)),
        }
    }
}

pub(crate) fn population_seed(seed: u32, center: ChunkCoord) -> u32 {
    let mut random = MtRandom::new(seed);
    let x_scale = odd_scale(random.next_positive_int());
    let z_scale = odd_scale(random.next_positive_int());
    let mixed = center
        .x()
        .wrapping_mul(x_scale)
        .wrapping_add(center.z().wrapping_mul(z_scale));
    u32::from_ne_bytes(mixed.to_ne_bytes()) ^ seed
}

pub(crate) fn population_random(seed: u32, center: ChunkCoord) -> MtRandom {
    MtRandom::new(population_seed(seed, center))
}

const fn odd_scale(value: u32) -> i32 {
    let value = value as i32;
    (value / 2).wrapping_mul(2).wrapping_add(1)
}

fn top_solid_for_population(id: u16, include_water: bool) -> bool {
    match id {
        0 | 10 | 11 => false,
        8 | 9 => include_water,
        _ => true,
    }
}

fn generation_height_map(states: &[u16]) -> Vec<u8> {
    let mut heights = vec![0_u8; CHUNK_COLUMN_COUNT];
    for z in 0..16 {
        for x in 0..16 {
            let mut height = WORLD_HEIGHT;
            while height > 0 {
                let id = states[block_index(x, height - 1, z)] >> 4;
                if generation_height_blocking(id) {
                    break;
                }
                height -= 1;
            }
            heights[x + z * 16] = height as u8;
        }
    }
    heights
}

/// `Block::mLightBlock[id] > 0` over the states reachable before population starts.
///
/// Every pre-population terrain/surface/cave state in the 0.15.10 Overworld has non-zero light
/// blocking except air and the Swamp surface prepass water-lily. Water and ice explicitly use
/// light-block 3; lava and ordinary solid terrain use non-zero/max light blocking.
const fn generation_height_blocking(id: u16) -> bool {
    !matches!(id, 0 | 111)
}

pub(crate) const fn state(block_id: u16, data: u8) -> u16 {
    (block_id << 4) | data as u16
}

const fn block_index(x: usize, y: usize, z: usize) -> usize {
    (y << 8) | (z << 4) | x
}

fn neighborhood_index(offset_x: i32, offset_z: i32) -> Option<usize> {
    if !(-1..=1).contains(&offset_x) || !(-1..=1).contains(&offset_z) {
        return None;
    }
    Some((offset_z + 1) as usize * 3 + (offset_x + 1) as usize)
}
