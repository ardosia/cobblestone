use std::sync::Arc;

use super::{
    CHUNK_COLUMN_COUNT, ChunkCoord, ChunkPatch, MAX_POINT_BLOCK_CHANGES, WorldChangeKind,
    WorldStore, WorldStoreError, biome_id, biome_id_is_supported, linear_index_to_extra_key,
    recalculate_column_height, validate_block_index, validate_column_index, validate_light,
    validate_state, write_nibble,
};

impl WorldStore {
    pub fn apply_patch(
        &self,
        position: ChunkCoord,
        patch: ChunkPatch,
    ) -> Result<(), WorldStoreError> {
        for &(index, state) in &patch.blocks {
            validate_block_index(index)?;
            validate_state(state)?;
        }
        for &(index, word) in &patch.biomes {
            validate_column_index(index)?;
            let id = biome_id(word);
            if !biome_id_is_supported(id) {
                return Err(WorldStoreError::InvalidBiome(id));
            }
        }
        for &(index, _) in &patch.extra_data {
            validate_block_index(index)?;
        }
        for &(index, level) in patch.sky_light.iter().chain(&patch.block_light) {
            validate_block_index(index)?;
            validate_light(level)?;
        }

        let terrain_changed =
            !(patch.blocks.is_empty() && patch.biomes.is_empty() && patch.extra_data.is_empty());
        let light_changed = !(patch.sky_light.is_empty() && patch.block_light.is_empty());
        let change_kind = if !terrain_changed && !light_changed {
            None
        } else if !light_changed
            && patch.biomes.is_empty()
            && patch.extra_data.is_empty()
            && patch.blocks.len() <= MAX_POINT_BLOCK_CHANGES
        {
            Some(WorldChangeKind::Blocks(patch.blocks.clone()))
        } else {
            Some(WorldChangeKind::FullChunk)
        };

        self.with_chunk_mut(position, |chunk| {
            if chunk.terrain_revision != patch.expected_terrain_revision {
                return Err(WorldStoreError::TerrainRevisionConflict {
                    expected: patch.expected_terrain_revision,
                    actual: chunk.terrain_revision,
                });
            }
            if chunk.light_revision != patch.expected_light_revision {
                return Err(WorldStoreError::LightRevisionConflict {
                    expected: patch.expected_light_revision,
                    actual: chunk.light_revision,
                });
            }

            let expected_terrain_next = if terrain_changed {
                chunk
                    .terrain_revision
                    .checked_add(1)
                    .ok_or(WorldStoreError::TerrainRevisionExhausted)?
            } else {
                chunk.terrain_revision
            };
            if patch.next_terrain_revision != expected_terrain_next {
                return Err(WorldStoreError::InvalidTerrainRevisionTransition {
                    expected_next: expected_terrain_next,
                    requested: patch.next_terrain_revision,
                });
            }

            let expected_light_next = if light_changed {
                chunk
                    .light_revision
                    .checked_add(1)
                    .ok_or(WorldStoreError::LightRevisionExhausted)?
            } else {
                chunk.light_revision
            };
            if patch.next_light_revision != expected_light_next {
                return Err(WorldStoreError::InvalidLightRevisionTransition {
                    expected_next: expected_light_next,
                    requested: patch.next_light_revision,
                });
            }

            let data = Arc::make_mut(&mut chunk.data);
            let mut touched_columns = [false; CHUNK_COLUMN_COUNT];

            for &(index, state) in &patch.blocks {
                let index = usize::from(index);
                if data.states[index] != state {
                    data.states[index] = state;
                    touched_columns[index & 0xff] = true;
                }
            }
            for &(index, biome) in &patch.biomes {
                data.biomes[usize::from(index)] = biome;
            }
            for &(index, value) in &patch.extra_data {
                let key = linear_index_to_extra_key(index);
                if value == 0 {
                    data.extra_data.remove(&key);
                } else {
                    data.extra_data.insert(key, value);
                }
            }
            for &(index, level) in &patch.sky_light {
                write_nibble(&mut data.sky_light, usize::from(index), level);
            }
            for &(index, level) in &patch.block_light {
                write_nibble(&mut data.block_light, usize::from(index), level);
            }
            for (column, touched) in touched_columns.into_iter().enumerate() {
                if touched {
                    recalculate_column_height(data, column);
                }
            }

            chunk.terrain_revision = patch.next_terrain_revision;
            chunk.light_revision = patch.next_light_revision;
            Ok(())
        })?;

        if let Some(kind) = change_kind {
            self.record_change(position, kind);
        }
        Ok(())
    }
}
