use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::RegionId;

pub const CHUNK_EDGE: usize = 16;
pub const WORLD_HEIGHT: usize = 128;
pub const CHUNK_BLOCK_COUNT: usize = CHUNK_EDGE * CHUNK_EDGE * WORLD_HEIGHT;
pub const CHUNK_NIBBLE_BYTES: usize = CHUNK_BLOCK_COUNT / 2;
pub const CHUNK_COLUMN_COUNT: usize = CHUNK_EDGE * CHUNK_EDGE;
pub const REGION_CHUNK_EDGE: i32 = 8;
pub const MAX_LEGACY_STATE_ID: u16 = 0x0fff;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct ChunkCoord {
    x: i32,
    z: i32,
}

impl ChunkCoord {
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    pub const fn x(self) -> i32 {
        self.x
    }

    pub const fn z(self) -> i32 {
        self.z
    }
}

#[derive(Debug, Clone)]
struct ChunkData {
    states: Vec<u16>,
    sky_light: Vec<u8>,
    block_light: Vec<u8>,
    biomes: Vec<u8>,
    height_map: Vec<u8>,
    extra_data: BTreeMap<u16, u16>,
}

impl ChunkData {
    fn empty(biome: u8) -> Self {
        Self {
            states: vec![0; CHUNK_BLOCK_COUNT],
            sky_light: vec![0; CHUNK_NIBBLE_BYTES],
            block_light: vec![0; CHUNK_NIBBLE_BYTES],
            biomes: vec![biome; CHUNK_COLUMN_COUNT],
            height_map: vec![0; CHUNK_COLUMN_COUNT],
            extra_data: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct ChunkRecord {
    terrain_revision: u64,
    light_revision: u64,
    persisted_terrain_revision: Option<u64>,
    persisted_light_revision: Option<u64>,
    persisted_lifecycle_flags: Option<u8>,
    pin_count: u32,
    lifecycle_flags: u8,
    data: Arc<ChunkData>,
}

impl ChunkRecord {
    fn empty(biome: u8) -> Self {
        Self {
            terrain_revision: 0,
            light_revision: 0,
            persisted_terrain_revision: None,
            persisted_light_revision: None,
            persisted_lifecycle_flags: None,
            pin_count: 0,
            lifecycle_flags: 0,
            data: Arc::new(ChunkData::empty(biome)),
        }
    }

    fn is_dirty(&self) -> bool {
        self.persisted_terrain_revision != Some(self.terrain_revision)
            || self.persisted_light_revision != Some(self.light_revision)
            || self.persisted_lifecycle_flags != Some(self.lifecycle_flags)
    }
}

#[derive(Debug, Default)]
struct RegionShard {
    chunks: RwLock<HashMap<ChunkCoord, ChunkRecord>>,
}

#[derive(Debug, Clone)]
pub struct ChunkSnapshot {
    position: ChunkCoord,
    terrain_revision: u64,
    light_revision: u64,
    lifecycle_flags: u8,
    data: Arc<ChunkData>,
}

impl ChunkSnapshot {
    pub const fn position(&self) -> ChunkCoord {
        self.position
    }

    pub const fn terrain_revision(&self) -> u64 {
        self.terrain_revision
    }

    pub const fn light_revision(&self) -> u64 {
        self.light_revision
    }

    pub const fn lifecycle_flags(&self) -> u8 {
        self.lifecycle_flags
    }

    pub fn states(&self) -> &[u16] {
        &self.data.states
    }

    pub fn sky_light(&self) -> &[u8] {
        &self.data.sky_light
    }

    pub fn block_light(&self) -> &[u8] {
        &self.data.block_light
    }

    pub fn biomes(&self) -> &[u8] {
        &self.data.biomes
    }

    pub fn height_map(&self) -> &[u8] {
        &self.data.height_map
    }

    pub fn extra_data(&self) -> &BTreeMap<u16, u16> {
        &self.data.extra_data
    }
}

#[derive(Debug, Clone)]
pub struct ChunkImport {
    pub terrain_revision: u64,
    pub light_revision: u64,
    pub lifecycle_flags: u8,
    pub states: Vec<u16>,
    pub sky_light: Vec<u8>,
    pub block_light: Vec<u8>,
    pub biomes: Vec<u8>,
    pub height_map: Vec<u8>,
    pub extra_data: BTreeMap<u16, u16>,
}

#[derive(Debug, Clone, Default)]
pub struct ChunkPatch {
    pub expected_terrain_revision: u64,
    pub next_terrain_revision: u64,
    pub expected_light_revision: u64,
    pub next_light_revision: u64,
    pub blocks: Vec<(u16, u16)>,
    pub biomes: Vec<(u8, u8)>,
    pub extra_data: Vec<(u16, u16)>,
    pub sky_light: Vec<(u16, u8)>,
    pub block_light: Vec<(u16, u8)>,
}

pub const WORLD_CHANGE_LOG_CAPACITY: usize = 8192;
pub const MAX_POINT_BLOCK_CHANGES: usize = 256;
pub const CHUNK_LIFECYCLE_GENERATED: u8 = 0x01;
pub const CHUNK_LIFECYCLE_POPULATED: u8 = 0x02;
pub const CHUNK_LIFECYCLE_LIGHT_POPULATED: u8 = 0x04;
pub const CHUNK_LIFECYCLE_MASK: u8 =
    CHUNK_LIFECYCLE_GENERATED | CHUNK_LIFECYCLE_POPULATED | CHUNK_LIFECYCLE_LIGHT_POPULATED;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ChunkEviction {
    Missing,
    Pinned { pins: u32 },
    Dirty,
    Evicted,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum WorldChangeKind {
    Blocks(Vec<(u16, u16)>),
    FullChunk,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WorldChange {
    sequence: u64,
    position: ChunkCoord,
    kind: WorldChangeKind,
}

impl WorldChange {
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn position(&self) -> ChunkCoord {
        self.position
    }

    pub const fn kind(&self) -> &WorldChangeKind {
        &self.kind
    }
}

#[derive(Debug, Clone)]
pub struct WorldChangeLogSnapshot {
    oldest_sequence: u64,
    latest_sequence: u64,
    changes: Vec<WorldChange>,
}

impl WorldChangeLogSnapshot {
    pub const fn oldest_sequence(&self) -> u64 {
        self.oldest_sequence
    }

    pub const fn latest_sequence(&self) -> u64 {
        self.latest_sequence
    }

    pub fn changes(&self) -> &[WorldChange] {
        &self.changes
    }

    pub fn cursor_is_stale(&self, cursor: u64) -> bool {
        cursor.saturating_add(1) < self.oldest_sequence
    }
}

#[derive(Debug)]
struct WorldChangeLog {
    next_sequence: u64,
    changes: VecDeque<WorldChange>,
}

impl Default for WorldChangeLog {
    fn default() -> Self {
        Self {
            next_sequence: 1,
            changes: VecDeque::with_capacity(WORLD_CHANGE_LOG_CAPACITY),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum WorldStoreError {
    ChunkMissing { x: i32, z: i32 },
    InvalidState(u16),
    InvalidBlockIndex(u16),
    InvalidColumnIndex(u8),
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

    pub fn current_change_sequence(&self) -> u64 {
        lock_changes(&self.changes).next_sequence.saturating_sub(1)
    }

    pub fn change_log_snapshot(&self) -> WorldChangeLogSnapshot {
        let changes = lock_changes(&self.changes);
        let latest_sequence = changes.next_sequence.saturating_sub(1);
        let oldest_sequence = changes
            .changes
            .front()
            .map(WorldChange::sequence)
            .unwrap_or_else(|| latest_sequence.saturating_add(1));

        WorldChangeLogSnapshot {
            oldest_sequence,
            latest_sequence,
            changes: changes.changes.iter().cloned().collect(),
        }
    }

    pub fn prune_changes_through(&self, sequence: u64) {
        let mut changes = lock_changes(&self.changes);
        while changes
            .changes
            .front()
            .is_some_and(|change| change.sequence <= sequence)
        {
            changes.changes.pop_front();
        }
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

    pub fn import_chunk(
        &self,
        position: ChunkCoord,
        import: ChunkImport,
    ) -> Result<(), WorldStoreError> {
        validate_import(&import)?;
        let region = self.region_or_create(position);
        let mut chunks = write_lock(&region.chunks);
        chunks.insert(
            position,
            ChunkRecord {
                terrain_revision: import.terrain_revision,
                light_revision: import.light_revision,
                persisted_terrain_revision: Some(import.terrain_revision),
                persisted_light_revision: Some(import.light_revision),
                persisted_lifecycle_flags: Some(import.lifecycle_flags),
                pin_count: 0,
                lifecycle_flags: import.lifecycle_flags,
                data: Arc::new(ChunkData {
                    states: import.states,
                    sky_light: import.sky_light,
                    block_light: import.block_light,
                    biomes: import.biomes,
                    height_map: import.height_map,
                    extra_data: import.extra_data,
                }),
            },
        );
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

    pub fn apply_patch(
        &self,
        position: ChunkCoord,
        patch: ChunkPatch,
    ) -> Result<(), WorldStoreError> {
        for &(index, state) in &patch.blocks {
            validate_block_index(index)?;
            validate_state(state)?;
        }
        for &(index, _) in &patch.biomes {
            validate_column_index(index)?;
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

    fn record_change(&self, position: ChunkCoord, kind: WorldChangeKind) {
        let mut changes = lock_changes(&self.changes);
        let sequence = changes.next_sequence;
        changes.next_sequence = changes
            .next_sequence
            .checked_add(1)
            .expect("world change sequence exhausted");

        if changes.changes.len() == WORLD_CHANGE_LOG_CAPACITY {
            changes.changes.pop_front();
        }
        changes.changes.push_back(WorldChange {
            sequence,
            position,
            kind,
        });
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

fn lock_changes(lock: &Mutex<WorldChangeLog>) -> MutexGuard<'_, WorldChangeLog> {
    match lock.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
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
    use super::{
        CHUNK_LIFECYCLE_GENERATED, CHUNK_LIFECYCLE_POPULATED, ChunkCoord, ChunkEviction,
        ChunkPatch, WorldChangeKind, WorldStore,
    };

    #[test]
    fn snapshots_are_immutable_across_later_writes() {
        let store = WorldStore::new();
        let pos = ChunkCoord::new(0, 0);
        store.ensure_chunk(pos, 1);
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
        store.ensure_chunk(pos, 1);

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
        store.ensure_chunk(pos, 1);
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
    fn pins_dirty_watermarks_and_lifecycle_gate_eviction() {
        let store = WorldStore::new();
        let pos = ChunkCoord::new(4, -2);
        store.ensure_chunk(pos, 1);

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
