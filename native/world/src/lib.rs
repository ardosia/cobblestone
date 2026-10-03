mod biome;
mod change_log;
mod chunk;
mod patch;
mod residency;
mod revision;
mod scalar;

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};

use cobblestone_runtime::RegionId;

pub use biome::{
    BIOME_COLOR_MASK, biome_color, biome_id, biome_id_is_supported, default_biome_word,
    with_biome_color, with_biome_id,
};
use change_log::WorldChangeLog;
pub use change_log::{
    MAX_POINT_BLOCK_CHANGES, WORLD_CHANGE_LOG_CAPACITY, WorldChange, WorldChangeKind,
    WorldChangeLogSnapshot,
};
pub use chunk::{
    CHUNK_BLOCK_COUNT, CHUNK_COLUMN_COUNT, CHUNK_EDGE, CHUNK_LIFECYCLE_GENERATED,
    CHUNK_LIFECYCLE_LIGHT_POPULATED, CHUNK_LIFECYCLE_MASK, CHUNK_LIFECYCLE_POPULATED,
    CHUNK_NIBBLE_BYTES, ChunkCoord, ChunkEviction, ChunkImport, ChunkPatch, ChunkSnapshot,
    MAX_LEGACY_STATE_ID, REGION_CHUNK_EDGE, WORLD_HEIGHT,
};
use chunk::{ChunkData, ChunkRecord, RegionShard};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum WorldStoreError {
    ChunkMissing { x: i32, z: i32 },
    InvalidState(u16),
    InvalidBlockIndex(u16),
    InvalidColumnIndex(u8),
    InvalidBiome(u8),
    InvalidLight(u8),
    InvalidLayerRange { start_y: u8, count: u8 },
    InvalidImport(&'static str),
    InvalidLifecycleFlags(u8),
    PinCountExhausted,
    ChunkNotPinned,
    PersistedTerrainRevisionAhead { persisted: u64, current: u64 },
    PersistedLightRevisionAhead { persisted: u64, current: u64 },
    PersistedTerrainRevisionRegression { previous: u64, requested: u64 },
    PersistedLightRevisionRegression { previous: u64, requested: u64 },
    TerrainRevisionConflict { expected: u64, actual: u64 },
    LightRevisionConflict { expected: u64, actual: u64 },
    InvalidTerrainRevisionTransition { expected_next: u64, requested: u64 },
    InvalidLightRevisionTransition { expected_next: u64, requested: u64 },
    TerrainRevisionExhausted,
    LightRevisionExhausted,
}

impl fmt::Display for WorldStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ChunkMissing { x, z } => write!(f, "native chunk {x}:{z} is not resident"),
            Self::InvalidState(state) => write!(f, "legacy state id {state} exceeds 12 bits"),
            Self::InvalidBlockIndex(index) => {
                write!(f, "chunk block index {index} is out of range")
            }
            Self::InvalidColumnIndex(index) => {
                write!(f, "chunk column index {index} is out of range")
            }
            Self::InvalidBiome(id) => write!(f, "unsupported MCPE 0.15.10 biome id {id}"),
            Self::InvalidLight(level) => write!(f, "light level {level} is out of range"),
            Self::InvalidLayerRange { start_y, count } => {
                write!(
                    f,
                    "chunk layer range start={start_y} count={count} is out of range"
                )
            }
            Self::InvalidImport(field) => {
                write!(f, "native chunk import has invalid {field} length")
            }
            Self::InvalidLifecycleFlags(flags) => {
                write!(f, "native chunk lifecycle flags 0x{flags:02x} are invalid")
            }
            Self::PinCountExhausted => write!(f, "native chunk pin count exhausted"),
            Self::ChunkNotPinned => write!(f, "native chunk is not pinned"),
            Self::PersistedTerrainRevisionAhead { persisted, current } => write!(
                f,
                "persisted terrain revision {persisted} exceeds current terrain revision {current}"
            ),
            Self::PersistedLightRevisionAhead { persisted, current } => write!(
                f,
                "persisted light revision {persisted} exceeds current light revision {current}"
            ),
            Self::PersistedTerrainRevisionRegression {
                previous,
                requested,
            } => write!(
                f,
                "persisted terrain revision regressed: previous {previous}, requested {requested}"
            ),
            Self::PersistedLightRevisionRegression {
                previous,
                requested,
            } => write!(
                f,
                "persisted light revision regressed: previous {previous}, requested {requested}"
            ),
            Self::TerrainRevisionConflict { expected, actual } => {
                write!(
                    f,
                    "chunk terrain revision changed: expected {expected}, actual {actual}"
                )
            }
            Self::LightRevisionConflict { expected, actual } => {
                write!(
                    f,
                    "chunk light revision changed: expected {expected}, actual {actual}"
                )
            }
            Self::InvalidTerrainRevisionTransition {
                expected_next,
                requested,
            } => write!(
                f,
                "invalid terrain revision transition: expected next {expected_next}, requested {requested}"
            ),
            Self::InvalidLightRevisionTransition {
                expected_next,
                requested,
            } => write!(
                f,
                "invalid light revision transition: expected next {expected_next}, requested {requested}"
            ),
            Self::TerrainRevisionExhausted => write!(f, "chunk terrain revision space exhausted"),
            Self::LightRevisionExhausted => write!(f, "chunk light revision space exhausted"),
        }
    }
}

impl std::error::Error for WorldStoreError {}

#[derive(Debug, Default)]
pub struct WorldStore {
    regions: RwLock<HashMap<RegionId, Arc<RegionShard>>>,
    changes: Mutex<WorldChangeLog>,
}

impl WorldStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self, position: ChunkCoord) -> Result<ChunkSnapshot, WorldStoreError> {
        self.with_chunk(position, |chunk| {
            Ok(ChunkSnapshot {
                position,
                terrain_revision: chunk.terrain_revision,
                light_revision: chunk.light_revision,
                lifecycle_flags: chunk.lifecycle_flags,
                data: Arc::clone(&chunk.data),
            })
        })
    }

    fn region_for(position: ChunkCoord) -> RegionId {
        RegionId::new(
            position.x.div_euclid(REGION_CHUNK_EDGE),
            position.z.div_euclid(REGION_CHUNK_EDGE),
        )
    }

    fn region_or_create(&self, position: ChunkCoord) -> Arc<RegionShard> {
        let id = Self::region_for(position);
        if let Some(region) = read_lock(&self.regions).get(&id).cloned() {
            return region;
        }
        let mut regions = write_lock(&self.regions);
        Arc::clone(
            regions
                .entry(id)
                .or_insert_with(|| Arc::new(RegionShard::default())),
        )
    }

    fn region(&self, position: ChunkCoord) -> Result<Arc<RegionShard>, WorldStoreError> {
        let id = Self::region_for(position);
        read_lock(&self.regions)
            .get(&id)
            .cloned()
            .ok_or(WorldStoreError::ChunkMissing {
                x: position.x,
                z: position.z,
            })
    }

    fn with_chunk<T>(
        &self,
        position: ChunkCoord,
        operation: impl FnOnce(&ChunkRecord) -> Result<T, WorldStoreError>,
    ) -> Result<T, WorldStoreError> {
        let region = self.region(position)?;
        let chunks = read_lock(&region.chunks);
        let chunk = chunks.get(&position).ok_or(WorldStoreError::ChunkMissing {
            x: position.x,
            z: position.z,
        })?;
        operation(chunk)
    }

    fn with_chunk_mut<T>(
        &self,
        position: ChunkCoord,
        operation: impl FnOnce(&mut ChunkRecord) -> Result<T, WorldStoreError>,
    ) -> Result<T, WorldStoreError> {
        let region = self.region(position)?;
        let mut chunks = write_lock(&region.chunks);
        let chunk = chunks
            .get_mut(&position)
            .ok_or(WorldStoreError::ChunkMissing {
                x: position.x,
                z: position.z,
            })?;
        operation(chunk)
    }
}

fn validate_import(import: &ChunkImport) -> Result<(), WorldStoreError> {
    validate_lifecycle_flags(import.lifecycle_flags)?;
    if import.states.len() != CHUNK_BLOCK_COUNT {
        return Err(WorldStoreError::InvalidImport("state"));
    }
    if import.sky_light.len() != CHUNK_NIBBLE_BYTES {
        return Err(WorldStoreError::InvalidImport("sky-light"));
    }
    if import.block_light.len() != CHUNK_NIBBLE_BYTES {
        return Err(WorldStoreError::InvalidImport("block-light"));
    }
    if import.biomes.len() != CHUNK_COLUMN_COUNT {
        return Err(WorldStoreError::InvalidImport("biome"));
    }
    for &word in &import.biomes {
        if !biome_id_is_supported(biome_id(word)) {
            return Err(WorldStoreError::InvalidBiome(biome_id(word)));
        }
    }
    if import.height_map.len() != CHUNK_COLUMN_COUNT {
        return Err(WorldStoreError::InvalidImport("height-map"));
    }
    for &state in &import.states {
        validate_state(state)?;
    }
    Ok(())
}

fn block_index(x: u8, y: u8, z: u8) -> Result<usize, WorldStoreError> {
    if usize::from(x) >= CHUNK_EDGE
        || usize::from(z) >= CHUNK_EDGE
        || usize::from(y) >= WORLD_HEIGHT
    {
        return Err(WorldStoreError::InvalidBlockIndex(
            (u16::from(y) << 8) | (u16::from(z) << 4) | u16::from(x),
        ));
    }
    Ok((usize::from(y) << 8) | (usize::from(z) << 4) | usize::from(x))
}

fn column_index(x: u8, z: u8) -> Result<usize, WorldStoreError> {
    if usize::from(x) >= CHUNK_EDGE || usize::from(z) >= CHUNK_EDGE {
        return Err(WorldStoreError::InvalidColumnIndex((z << 4) | x));
    }
    Ok((usize::from(z) << 4) | usize::from(x))
}

fn extra_key(x: u8, y: u8, z: u8) -> Result<u16, WorldStoreError> {
    block_index(x, y, z)?;
    Ok((u16::from(z) << 12) | (u16::from(x) << 8) | u16::from(y))
}

fn linear_index_to_extra_key(index: u16) -> u16 {
    let x = index & 0x0f;
    let z = (index >> 4) & 0x0f;
    let y = (index >> 8) & 0x7f;
    (z << 12) | (x << 8) | y
}

fn validate_block_index(index: u16) -> Result<(), WorldStoreError> {
    if usize::from(index) >= CHUNK_BLOCK_COUNT {
        Err(WorldStoreError::InvalidBlockIndex(index))
    } else {
        Ok(())
    }
}

fn validate_column_index(index: u8) -> Result<(), WorldStoreError> {
    if usize::from(index) >= CHUNK_COLUMN_COUNT {
        Err(WorldStoreError::InvalidColumnIndex(index))
    } else {
        Ok(())
    }
}

fn validate_lifecycle_flags(flags: u8) -> Result<(), WorldStoreError> {
    if flags & !CHUNK_LIFECYCLE_MASK != 0 {
        Err(WorldStoreError::InvalidLifecycleFlags(flags))
    } else {
        Ok(())
    }
}

fn validate_state(state: u16) -> Result<(), WorldStoreError> {
    if state > MAX_LEGACY_STATE_ID {
        Err(WorldStoreError::InvalidState(state))
    } else {
        Ok(())
    }
}

fn validate_light(level: u8) -> Result<(), WorldStoreError> {
    if level > 0x0f {
        Err(WorldStoreError::InvalidLight(level))
    } else {
        Ok(())
    }
}

fn read_nibble(bytes: &[u8], index: usize) -> u8 {
    let value = bytes[index >> 1];
    if index & 1 == 0 {
        value & 0x0f
    } else {
        (value >> 4) & 0x0f
    }
}

fn write_nibble(bytes: &mut [u8], index: usize, value: u8) {
    let byte = &mut bytes[index >> 1];
    if index & 1 == 0 {
        *byte = (*byte & 0xf0) | value;
    } else {
        *byte = (*byte & 0x0f) | (value << 4);
    }
}

fn recalculate_all_heights(data: &mut ChunkData) {
    for column in 0..CHUNK_COLUMN_COUNT {
        recalculate_column_height(data, column);
    }
}

fn recalculate_column_height(data: &mut ChunkData, column: usize) {
    for y in (0..WORLD_HEIGHT).rev() {
        if data.states[y * CHUNK_COLUMN_COUNT + column] != 0 {
            data.height_map[column] = u8::try_from(y).expect("fixed world height fits u8");
            return;
        }
    }
    data.height_map[column] = 0;
}

fn read_lock<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    match lock.read() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn write_lock<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    match lock.write() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        CHUNK_BLOCK_COUNT, CHUNK_COLUMN_COUNT, CHUNK_LIFECYCLE_GENERATED,
        CHUNK_LIFECYCLE_POPULATED, CHUNK_NIBBLE_BYTES, ChunkCoord, ChunkEviction, ChunkImport,
        ChunkPatch, WorldChangeKind, WorldStore,
    };

    #[test]
    fn snapshots_are_immutable_across_later_writes() {
        let store = WorldStore::new();
        let pos = ChunkCoord::new(0, 0);
        store.ensure_chunk(pos, 1).unwrap();
        store.set_block_state(pos, 1, 2, 3, 0x10).unwrap();
        let first = store.snapshot(pos).unwrap();

        store.set_block_state(pos, 1, 2, 3, 0x20).unwrap();

        assert_eq!(first.states()[(2 << 8) | (3 << 4) | 1], 0x10);
        assert_eq!(store.block_state(pos, 1, 2, 3).unwrap(), 0x20);
    }

    #[test]
    fn batch_patch_advances_revision_domains_once() {
        let store = WorldStore::new();
        let pos = ChunkCoord::new(-1, 7);
        store.ensure_chunk(pos, 1).unwrap();

        store
            .apply_patch(
                pos,
                ChunkPatch {
                    expected_terrain_revision: 0,
                    next_terrain_revision: 1,
                    expected_light_revision: 0,
                    next_light_revision: 1,
                    blocks: vec![(0, 0x10), (1, 0x20)],
                    sky_light: vec![(0, 15)],
                    ..ChunkPatch::default()
                },
            )
            .unwrap();

        assert_eq!(store.terrain_revision(pos).unwrap(), 1);
        assert_eq!(store.light_revision(pos).unwrap(), 1);
    }

    #[test]
    fn change_log_keeps_point_blocks_and_escalates_light_to_full_chunk() {
        let store = WorldStore::new();
        let pos = ChunkCoord::new(2, -3);
        store.ensure_chunk(pos, 1).unwrap();
        let initial = store.current_change_sequence();

        store
            .apply_patch(
                pos,
                ChunkPatch {
                    expected_terrain_revision: 0,
                    next_terrain_revision: 1,
                    expected_light_revision: 0,
                    next_light_revision: 0,
                    blocks: vec![(0x0201, 0x32)],
                    ..ChunkPatch::default()
                },
            )
            .unwrap();
        store
            .apply_patch(
                pos,
                ChunkPatch {
                    expected_terrain_revision: 1,
                    next_terrain_revision: 1,
                    expected_light_revision: 0,
                    next_light_revision: 1,
                    block_light: vec![(0x0201, 7)],
                    ..ChunkPatch::default()
                },
            )
            .unwrap();

        let log = store.change_log_snapshot();
        assert_eq!(initial, 0);
        assert_eq!(log.latest_sequence(), 2);
        assert_eq!(log.changes().len(), 2);
        assert!(matches!(
            log.changes()[0].kind(),
            WorldChangeKind::Blocks(blocks) if blocks == &vec![(0x0201, 0x32)]
        ));
        assert!(matches!(
            log.changes()[1].kind(),
            WorldChangeKind::FullChunk
        ));

        store.prune_changes_through(1);
        let pruned = store.change_log_snapshot();
        assert_eq!(pruned.changes().len(), 1);
        assert_eq!(pruned.oldest_sequence(), 2);
    }

    #[test]
    fn import_if_absent_never_overwrites_live_residency() {
        let store = WorldStore::new();
        let pos = ChunkCoord::new(6, -7);
        let import = ChunkImport {
            terrain_revision: 4,
            light_revision: 3,
            lifecycle_flags: CHUNK_LIFECYCLE_GENERATED,
            states: {
                let mut states = vec![0; CHUNK_BLOCK_COUNT];
                states[0] = 0x32;
                states
            },
            sky_light: vec![0; CHUNK_NIBBLE_BYTES],
            block_light: vec![0; CHUNK_NIBBLE_BYTES],
            biomes: vec![1; CHUNK_COLUMN_COUNT],
            height_map: vec![0; CHUNK_COLUMN_COUNT],
            extra_data: BTreeMap::new(),
        };

        assert!(store.import_chunk_if_absent(pos, import.clone()).unwrap());
        assert!(!store.is_dirty(pos).unwrap());
        assert_eq!(store.block_state(pos, 0, 0, 0).unwrap(), 0x32);

        store.set_block_state(pos, 0, 0, 0, 0x45).unwrap();
        assert!(!store.import_chunk_if_absent(pos, import).unwrap());
        assert_eq!(store.block_state(pos, 0, 0, 0).unwrap(), 0x45);
    }

    #[test]
    fn pins_dirty_watermarks_and_lifecycle_gate_eviction() {
        let store = WorldStore::new();
        let pos = ChunkCoord::new(4, -2);
        store.ensure_chunk(pos, 1).unwrap();

        assert!(store.is_dirty(pos).unwrap());
        assert_eq!(store.try_evict_chunk(pos).unwrap(), ChunkEviction::Dirty);

        let flags = CHUNK_LIFECYCLE_GENERATED | CHUNK_LIFECYCLE_POPULATED;
        store.set_lifecycle_flags(pos, flags).unwrap();
        store.mark_persisted(pos, 0, 0, flags).unwrap();
        assert!(!store.is_dirty(pos).unwrap());

        store
            .set_lifecycle_flags(pos, CHUNK_LIFECYCLE_GENERATED)
            .unwrap();
        assert!(store.is_dirty(pos).unwrap());
        store.set_lifecycle_flags(pos, flags).unwrap();
        assert!(!store.is_dirty(pos).unwrap());

        assert_eq!(store.pin_chunk(pos).unwrap(), 1);
        assert_eq!(
            store.try_evict_chunk(pos).unwrap(),
            ChunkEviction::Pinned { pins: 1 }
        );
        assert_eq!(store.unpin_chunk(pos).unwrap(), 0);

        store
            .apply_patch(
                pos,
                ChunkPatch {
                    expected_terrain_revision: 0,
                    next_terrain_revision: 1,
                    expected_light_revision: 0,
                    next_light_revision: 0,
                    blocks: vec![(0, 0x10)],
                    ..ChunkPatch::default()
                },
            )
            .unwrap();
        assert!(store.is_dirty(pos).unwrap());
        assert_eq!(store.try_evict_chunk(pos).unwrap(), ChunkEviction::Dirty);

        store.mark_persisted(pos, 1, 0, flags).unwrap();
        assert!(!store.is_dirty(pos).unwrap());
        assert_eq!(store.try_evict_chunk(pos).unwrap(), ChunkEviction::Evicted);
        assert_eq!(store.try_evict_chunk(pos).unwrap(), ChunkEviction::Missing);
    }
}
