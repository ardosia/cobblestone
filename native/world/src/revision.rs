use super::{ChunkCoord, WorldStore, WorldStoreError};

impl WorldStore {
    pub fn terrain_revision(&self, position: ChunkCoord) -> Result<u64, WorldStoreError> {
        self.with_chunk(position, |chunk| Ok(chunk.terrain_revision))
    }

    pub fn light_revision(&self, position: ChunkCoord) -> Result<u64, WorldStoreError> {
        self.with_chunk(position, |chunk| Ok(chunk.light_revision))
    }

    pub fn commit_terrain_revision(
        &self,
        position: ChunkCoord,
        expected: u64,
        next: u64,
    ) -> Result<(), WorldStoreError> {
        self.with_chunk_mut(position, |chunk| {
            if chunk.terrain_revision != expected {
                return Err(WorldStoreError::TerrainRevisionConflict {
                    expected,
                    actual: chunk.terrain_revision,
                });
            }
            let expected_next = expected
                .checked_add(1)
                .ok_or(WorldStoreError::TerrainRevisionExhausted)?;
            if next != expected_next {
                return Err(WorldStoreError::InvalidTerrainRevisionTransition {
                    expected_next,
                    requested: next,
                });
            }
            chunk.terrain_revision = next;
            Ok(())
        })
    }

    pub fn commit_light_revision(
        &self,
        position: ChunkCoord,
        expected: u64,
        next: u64,
    ) -> Result<(), WorldStoreError> {
        self.with_chunk_mut(position, |chunk| {
            if chunk.light_revision != expected {
                return Err(WorldStoreError::LightRevisionConflict {
                    expected,
                    actual: chunk.light_revision,
                });
            }
            let expected_next = expected
                .checked_add(1)
                .ok_or(WorldStoreError::LightRevisionExhausted)?;
            if next != expected_next {
                return Err(WorldStoreError::InvalidLightRevisionTransition {
                    expected_next,
                    requested: next,
                });
            }
            chunk.light_revision = next;
            Ok(())
        })
    }
}
