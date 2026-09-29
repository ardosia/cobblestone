use std::collections::HashSet;
use std::sync::Arc;

use super::{
    ChunkCoord, ChunkEviction, ChunkImport, ChunkRecord, ChunkSnapshot, WorldStore,
    WorldStoreError, read_lock, validate_import, validate_lifecycle_flags, write_lock,
};

impl WorldStore {
    pub fn contains_chunk(&self, position: ChunkCoord) -> bool {
        let id = Self::region_for(position);
        let Some(region) = read_lock(&self.regions).get(&id).cloned() else {
            return false;
        };
        read_lock(&region.chunks).contains_key(&position)
    }

    pub fn ensure_chunk(&self, position: ChunkCoord, biome: u8) -> bool {
        let region = self.region_or_create(position);
        let mut chunks = write_lock(&region.chunks);
        if chunks.contains_key(&position) {
            return false;
        }
        chunks.insert(position, ChunkRecord::empty(biome));
        true
    }

    pub fn import_chunk_if_absent(
        &self,
        position: ChunkCoord,
        import: ChunkImport,
    ) -> Result<bool, WorldStoreError> {
        validate_import(&import)?;
        let region = self.region_or_create(position);
        let mut chunks = write_lock(&region.chunks);
        if chunks.contains_key(&position) {
            return Ok(false);
        }
        chunks.insert(position, ChunkRecord::from_import(import));
        Ok(true)
    }

    pub fn import_chunk(
        &self,
        position: ChunkCoord,
        import: ChunkImport,
    ) -> Result<(), WorldStoreError> {
        validate_import(&import)?;
        let region = self.region_or_create(position);
        let mut chunks = write_lock(&region.chunks);
        chunks.insert(position, ChunkRecord::from_import(import));
        Ok(())
    }

    pub fn lifecycle_flags(&self, position: ChunkCoord) -> Result<u8, WorldStoreError> {
        self.with_chunk(position, |chunk| Ok(chunk.lifecycle_flags))
    }

    pub fn set_lifecycle_flags(
        &self,
        position: ChunkCoord,
        flags: u8,
    ) -> Result<(), WorldStoreError> {
        validate_lifecycle_flags(flags)?;
        self.with_chunk_mut(position, |chunk| {
            chunk.lifecycle_flags = flags;
            Ok(())
        })
    }

    pub fn pin_chunk(&self, position: ChunkCoord) -> Result<u32, WorldStoreError> {
        self.with_chunk_mut(position, |chunk| {
            chunk.pin_count = chunk
                .pin_count
                .checked_add(1)
                .ok_or(WorldStoreError::PinCountExhausted)?;
            Ok(chunk.pin_count)
        })
    }

    pub fn unpin_chunk(&self, position: ChunkCoord) -> Result<u32, WorldStoreError> {
        self.with_chunk_mut(position, |chunk| {
            if chunk.pin_count == 0 {
                return Err(WorldStoreError::ChunkNotPinned);
            }
            chunk.pin_count -= 1;
            Ok(chunk.pin_count)
        })
    }

    pub fn pin_count(&self, position: ChunkCoord) -> Result<u32, WorldStoreError> {
        self.with_chunk(position, |chunk| Ok(chunk.pin_count))
    }

    pub fn total_pin_count(&self) -> u64 {
        let regions = read_lock(&self.regions);
        regions
            .values()
            .map(|region| {
                read_lock(&region.chunks)
                    .values()
                    .map(|chunk| u64::from(chunk.pin_count))
                    .sum::<u64>()
            })
            .sum()
    }

    pub fn is_dirty(&self, position: ChunkCoord) -> Result<bool, WorldStoreError> {
        self.with_chunk(position, |chunk| Ok(chunk.is_dirty()))
    }

    pub fn dirty_snapshots_excluding(
        &self,
        limit: usize,
        excluded: &HashSet<ChunkCoord>,
    ) -> Vec<ChunkSnapshot> {
        if limit == 0 {
            return Vec::new();
        }

        let regions = read_lock(&self.regions);
        let mut snapshots = Vec::with_capacity(limit);
        for region in regions.values() {
            let chunks = read_lock(&region.chunks);
            for (&position, chunk) in chunks.iter() {
                if chunk.is_dirty() && !excluded.contains(&position) {
                    snapshots.push(ChunkSnapshot {
                        position,
                        terrain_revision: chunk.terrain_revision,
                        light_revision: chunk.light_revision,
                        lifecycle_flags: chunk.lifecycle_flags,
                        data: Arc::clone(&chunk.data),
                    });
                    if snapshots.len() == limit {
                        return snapshots;
                    }
                }
            }
        }

        snapshots
    }

    pub fn persisted_revisions(
        &self,
        position: ChunkCoord,
    ) -> Result<(Option<u64>, Option<u64>), WorldStoreError> {
        self.with_chunk(position, |chunk| {
            Ok((
                chunk.persisted_terrain_revision,
                chunk.persisted_light_revision,
            ))
        })
    }

    pub fn mark_persisted(
        &self,
        position: ChunkCoord,
        terrain_revision: u64,
        light_revision: u64,
        lifecycle_flags: u8,
    ) -> Result<(), WorldStoreError> {
        validate_lifecycle_flags(lifecycle_flags)?;
        self.with_chunk_mut(position, |chunk| {
            if terrain_revision > chunk.terrain_revision {
                return Err(WorldStoreError::PersistedTerrainRevisionAhead {
                    persisted: terrain_revision,
                    current: chunk.terrain_revision,
                });
            }
            if light_revision > chunk.light_revision {
                return Err(WorldStoreError::PersistedLightRevisionAhead {
                    persisted: light_revision,
                    current: chunk.light_revision,
                });
            }
            if let Some(previous) = chunk.persisted_terrain_revision
                && terrain_revision < previous
            {
                return Err(WorldStoreError::PersistedTerrainRevisionRegression {
                    previous,
                    requested: terrain_revision,
                });
            }
            if let Some(previous) = chunk.persisted_light_revision
                && light_revision < previous
            {
                return Err(WorldStoreError::PersistedLightRevisionRegression {
                    previous,
                    requested: light_revision,
                });
            }

            chunk.persisted_terrain_revision = Some(terrain_revision);
            chunk.persisted_light_revision = Some(light_revision);
            chunk.persisted_lifecycle_flags = Some(lifecycle_flags);
            Ok(())
        })
    }

    pub fn try_evict_chunk(&self, position: ChunkCoord) -> Result<ChunkEviction, WorldStoreError> {
        let id = Self::region_for(position);
        let Some(region) = read_lock(&self.regions).get(&id).cloned() else {
            return Ok(ChunkEviction::Missing);
        };

        let empty_after = {
            let mut chunks = write_lock(&region.chunks);
            let Some(chunk) = chunks.get(&position) else {
                return Ok(ChunkEviction::Missing);
            };
            if chunk.pin_count != 0 {
                return Ok(ChunkEviction::Pinned {
                    pins: chunk.pin_count,
                });
            }
            if chunk.is_dirty() {
                return Ok(ChunkEviction::Dirty);
            }

            chunks.remove(&position);
            chunks.is_empty()
        };

        if empty_after {
            let mut regions = write_lock(&self.regions);
            if regions
                .get(&id)
                .is_some_and(|current| Arc::ptr_eq(current, &region))
                && read_lock(&region.chunks).is_empty()
            {
                regions.remove(&id);
            }
        }

        Ok(ChunkEviction::Evicted)
    }
}
