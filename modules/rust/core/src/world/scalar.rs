use std::sync::Arc;

use super::{
    CHUNK_BLOCK_COUNT, CHUNK_COLUMN_COUNT, CHUNK_EDGE, ChunkCoord, WORLD_HEIGHT, WorldStore,
    WorldStoreError, block_index, column_index, extra_key, read_nibble, recalculate_all_heights,
    recalculate_column_height, validate_light, validate_state, write_nibble,
};

impl WorldStore {
    pub fn block_state(
        &self,
        position: ChunkCoord,
        x: u8,
        y: u8,
        z: u8,
    ) -> Result<u16, WorldStoreError> {
        let index = block_index(x, y, z)?;
        self.with_chunk(position, |chunk| Ok(chunk.data.states[index]))
    }

    pub fn set_block_state(
        &self,
        position: ChunkCoord,
        x: u8,
        y: u8,
        z: u8,
        state: u16,
    ) -> Result<u16, WorldStoreError> {
        validate_state(state)?;
        let index = block_index(x, y, z)?;
        self.with_chunk_mut(position, |chunk| {
            let data = Arc::make_mut(&mut chunk.data);
            let previous = data.states[index];
            if previous != state {
                data.states[index] = state;
                recalculate_column_height(data, usize::from(z) * CHUNK_EDGE + usize::from(x));
            }
            Ok(previous)
        })
    }

    pub fn fill_layers(
        &self,
        position: ChunkCoord,
        start_y: u8,
        count: u8,
        state: u16,
    ) -> Result<(), WorldStoreError> {
        validate_state(state)?;
        let start = usize::from(start_y);
        let count = usize::from(count);
        if count == 0 || start >= WORLD_HEIGHT || start + count > WORLD_HEIGHT {
            return Err(WorldStoreError::InvalidLayerRange {
                start_y,
                count: count as u8,
            });
        }
        self.with_chunk_mut(position, |chunk| {
            let data = Arc::make_mut(&mut chunk.data);
            let first = start * CHUNK_COLUMN_COUNT;
            let last = (start + count) * CHUNK_COLUMN_COUNT;
            data.states[first..last].fill(state);
            recalculate_all_heights(data);
            Ok(())
        })
    }

    pub fn biome(&self, position: ChunkCoord, x: u8, z: u8) -> Result<u8, WorldStoreError> {
        let index = column_index(x, z)?;
        self.with_chunk(position, |chunk| Ok(chunk.data.biomes[index]))
    }

    pub fn set_biome(
        &self,
        position: ChunkCoord,
        x: u8,
        z: u8,
        biome: u8,
    ) -> Result<u8, WorldStoreError> {
        let index = column_index(x, z)?;
        self.with_chunk_mut(position, |chunk| {
            let data = Arc::make_mut(&mut chunk.data);
            let previous = data.biomes[index];
            data.biomes[index] = biome;
            Ok(previous)
        })
    }

    pub fn fill_biome(&self, position: ChunkCoord, biome: u8) -> Result<(), WorldStoreError> {
        self.with_chunk_mut(position, |chunk| {
            Arc::make_mut(&mut chunk.data).biomes.fill(biome);
            Ok(())
        })
    }

    pub fn sky_light(
        &self,
        position: ChunkCoord,
        x: u8,
        y: u8,
        z: u8,
    ) -> Result<u8, WorldStoreError> {
        let index = block_index(x, y, z)?;
        self.with_chunk(position, |chunk| {
            Ok(read_nibble(&chunk.data.sky_light, index))
        })
    }

    pub fn set_sky_light(
        &self,
        position: ChunkCoord,
        x: u8,
        y: u8,
        z: u8,
        level: u8,
    ) -> Result<u8, WorldStoreError> {
        validate_light(level)?;
        let index = block_index(x, y, z)?;
        self.with_chunk_mut(position, |chunk| {
            let data = Arc::make_mut(&mut chunk.data);
            let previous = read_nibble(&data.sky_light, index);
            write_nibble(&mut data.sky_light, index, level);
            Ok(previous)
        })
    }

    pub fn fill_sky_light_from(
        &self,
        position: ChunkCoord,
        y: u8,
        level: u8,
    ) -> Result<(), WorldStoreError> {
        validate_light(level)?;
        if usize::from(y) > WORLD_HEIGHT {
            return Err(WorldStoreError::InvalidLayerRange {
                start_y: y,
                count: 0,
            });
        }
        self.with_chunk_mut(position, |chunk| {
            let data = Arc::make_mut(&mut chunk.data);
            for index in usize::from(y) * CHUNK_COLUMN_COUNT..CHUNK_BLOCK_COUNT {
                write_nibble(&mut data.sky_light, index, level);
            }
            Ok(())
        })
    }

    pub fn block_light(
        &self,
        position: ChunkCoord,
        x: u8,
        y: u8,
        z: u8,
    ) -> Result<u8, WorldStoreError> {
        let index = block_index(x, y, z)?;
        self.with_chunk(position, |chunk| {
            Ok(read_nibble(&chunk.data.block_light, index))
        })
    }

    pub fn set_block_light(
        &self,
        position: ChunkCoord,
        x: u8,
        y: u8,
        z: u8,
        level: u8,
    ) -> Result<u8, WorldStoreError> {
        validate_light(level)?;
        let index = block_index(x, y, z)?;
        self.with_chunk_mut(position, |chunk| {
            let data = Arc::make_mut(&mut chunk.data);
            let previous = read_nibble(&data.block_light, index);
            write_nibble(&mut data.block_light, index, level);
            Ok(previous)
        })
    }

    pub fn height_map(&self, position: ChunkCoord, x: u8, z: u8) -> Result<u8, WorldStoreError> {
        let index = column_index(x, z)?;
        self.with_chunk(position, |chunk| Ok(chunk.data.height_map[index]))
    }

    pub fn recalculate_height_map(&self, position: ChunkCoord) -> Result<(), WorldStoreError> {
        self.with_chunk_mut(position, |chunk| {
            recalculate_all_heights(Arc::make_mut(&mut chunk.data));
            Ok(())
        })
    }

    pub fn block_extra_data(
        &self,
        position: ChunkCoord,
        x: u8,
        y: u8,
        z: u8,
    ) -> Result<u16, WorldStoreError> {
        let key = extra_key(x, y, z)?;
        self.with_chunk(position, |chunk| {
            Ok(*chunk.data.extra_data.get(&key).unwrap_or(&0))
        })
    }

    pub fn set_block_extra_data(
        &self,
        position: ChunkCoord,
        x: u8,
        y: u8,
        z: u8,
        value: u16,
    ) -> Result<u16, WorldStoreError> {
        let key = extra_key(x, y, z)?;
        self.with_chunk_mut(position, |chunk| {
            let data = Arc::make_mut(&mut chunk.data);
            let previous = *data.extra_data.get(&key).unwrap_or(&0);
            if value == 0 {
                data.extra_data.remove(&key);
            } else {
                data.extra_data.insert(key, value);
            }
            Ok(previous)
        })
    }
}
