use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock};

pub const CHUNK_EDGE: usize = 16;
pub const WORLD_HEIGHT: usize = 128;
pub const CHUNK_BLOCK_COUNT: usize = CHUNK_EDGE * CHUNK_EDGE * WORLD_HEIGHT;
pub const CHUNK_NIBBLE_BYTES: usize = CHUNK_BLOCK_COUNT / 2;
pub const CHUNK_COLUMN_COUNT: usize = CHUNK_EDGE * CHUNK_EDGE;
pub const REGION_CHUNK_EDGE: i32 = 8;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct ChunkCoord {
    pub(super) x: i32,
    pub(super) z: i32,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ChestItemStack {
    pub slot: u8,
    pub item_id: i16,
    pub damage: i16,
    pub count: u8,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ChestBlockEntity {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub items: Vec<ChestItemStack>,
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
pub(super) struct ChunkData {
    pub(super) states: Vec<u16>,
    pub(super) sky_light: Vec<u8>,
    pub(super) block_light: Vec<u8>,
    pub(super) biomes: Vec<u32>,
    pub(super) height_map: Vec<u8>,
    pub(super) extra_data: BTreeMap<u16, u16>,
    pub(super) chest_block_entities: Vec<ChestBlockEntity>,
}

impl ChunkData {
    pub(super) fn empty(biome_word: u32) -> Self {
        Self {
            states: vec![0; CHUNK_BLOCK_COUNT],
            sky_light: vec![0; CHUNK_NIBBLE_BYTES],
            block_light: vec![0; CHUNK_NIBBLE_BYTES],
            biomes: vec![biome_word; CHUNK_COLUMN_COUNT],
            height_map: vec![0; CHUNK_COLUMN_COUNT],
            extra_data: BTreeMap::new(),
            chest_block_entities: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct ChunkRecord {
    pub(super) terrain_revision: u64,
    pub(super) light_revision: u64,
    pub(super) persisted_terrain_revision: Option<u64>,
    pub(super) persisted_light_revision: Option<u64>,
    pub(super) persisted_lifecycle_flags: Option<u8>,
    pub(super) pin_count: u32,
    pub(super) lifecycle_flags: u8,
    pub(super) data: Arc<ChunkData>,
}

impl ChunkRecord {
    pub(super) fn empty(biome_word: u32) -> Self {
        Self {
            terrain_revision: 0,
            light_revision: 0,
            persisted_terrain_revision: None,
            persisted_light_revision: None,
            persisted_lifecycle_flags: None,
            pin_count: 0,
            lifecycle_flags: 0,
            data: Arc::new(ChunkData::empty(biome_word)),
        }
    }

    pub(super) fn from_import(import: ChunkImport) -> Self {
        Self {
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
                chest_block_entities: import.chest_block_entities,
            }),
        }
    }

    pub(super) fn is_dirty(&self) -> bool {
        self.persisted_terrain_revision != Some(self.terrain_revision)
            || self.persisted_light_revision != Some(self.light_revision)
            || self.persisted_lifecycle_flags != Some(self.lifecycle_flags)
    }
}

#[derive(Debug, Default)]
pub(super) struct RegionShard {
    pub(super) chunks: RwLock<HashMap<ChunkCoord, ChunkRecord>>,
}

#[derive(Debug, Clone)]
pub struct ChunkSnapshot {
    pub(super) position: ChunkCoord,
    pub(super) terrain_revision: u64,
    pub(super) light_revision: u64,
    pub(super) lifecycle_flags: u8,
    pub(super) data: Arc<ChunkData>,
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

    pub fn biomes(&self) -> &[u32] {
        &self.data.biomes
    }

    pub fn height_map(&self) -> &[u8] {
        &self.data.height_map
    }

    pub fn extra_data(&self) -> &BTreeMap<u16, u16> {
        &self.data.extra_data
    }

    pub fn chest_block_entities(&self) -> &[ChestBlockEntity] {
        &self.data.chest_block_entities
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
    pub biomes: Vec<u32>,
    pub height_map: Vec<u8>,
    pub extra_data: BTreeMap<u16, u16>,
    pub chest_block_entities: Vec<ChestBlockEntity>,
}

#[derive(Debug, Clone, Default)]
pub struct ChunkPatch {
    pub expected_terrain_revision: u64,
    pub next_terrain_revision: u64,
    pub expected_light_revision: u64,
    pub next_light_revision: u64,
    pub blocks: Vec<(u16, u16)>,
    pub biomes: Vec<(u8, u32)>,
    pub extra_data: Vec<(u16, u16)>,
    pub sky_light: Vec<(u16, u8)>,
    pub block_light: Vec<(u16, u8)>,
}

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
