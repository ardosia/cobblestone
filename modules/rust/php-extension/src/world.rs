use std::collections::HashMap;
mod block;
mod block_extra;
mod height;
mod lifecycle;
mod light;
mod patch;
mod registration;
mod residency;
mod revision;
mod snapshot;
mod storage;

use std::sync::{Arc, Mutex, MutexGuard};

use cobblestone_codec::{Protocol84ChunkSnapshot, RawPacket, encode_protocol84_full_chunk_data};
use cobblestone_core::{Arena, CHUNK_NIBBLE_BYTES, ChunkCoord, Handle, RuntimeId, WorldStore};
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::php_error;
use crate::runtime::current_runtime_id;

const MAX_PROTOCOL84_CACHE_ENTRIES: usize = 4096;

static WORLD_ARENA: Mutex<Arena<NativeWorld>> = Mutex::new(Arena::new());

struct NativeWorld {
    owner: RuntimeId,
    state: Arc<NativeWorldState>,
}

struct NativeWorldState {
    store: Arc<WorldStore>,
    protocol84_cache: Mutex<HashMap<ChunkCoord, CachedProtocol84Chunk>>,
    persistence: Mutex<Option<storage::NativeWorldPersistence>>,
}

struct CachedProtocol84Chunk {
    terrain_revision: u64,
    light_revision: u64,
    packet: RawPacket,
}

fn world_arena() -> MutexGuard<'static, Arena<NativeWorld>> {
    match WORLD_ARENA.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn handle(value: i64) -> PhpResult<Handle<NativeWorld>> {
    Handle::from_raw(value as u64).ok_or_else(|| php_error("invalid native world handle"))
}

fn position(x: i64, z: i64) -> PhpResult<ChunkCoord> {
    let x = i32::try_from(x).map_err(|_| php_error("chunk x must fit signed 32 bits"))?;
    let z = i32::try_from(z).map_err(|_| php_error("chunk z must fit signed 32 bits"))?;
    Ok(ChunkCoord::new(x, z))
}

fn byte(value: i64, field: &'static str) -> PhpResult<u8> {
    u8::try_from(value).map_err(|_| php_error(format!("{field} must fit one byte")))
}

fn local(value: i64, field: &'static str) -> PhpResult<u8> {
    let value = byte(value, field)?;
    if value > 15 {
        return Err(php_error(format!("{field} must be in range 0..15")));
    }
    Ok(value)
}

fn block_y(value: i64) -> PhpResult<u8> {
    let value = byte(value, "block y")?;
    if value > 127 {
        return Err(php_error("block y must be in range 0..127"));
    }
    Ok(value)
}

fn fill_y(value: i64) -> PhpResult<u8> {
    let value = byte(value, "fill y")?;
    if value > 128 {
        return Err(php_error("fill y must be in range 0..128"));
    }
    Ok(value)
}

fn state_id(value: i64) -> PhpResult<u16> {
    let value = u16::try_from(value).map_err(|_| php_error("block state id must fit 16 bits"))?;
    if value > 0x0fff {
        return Err(php_error("block state id must be in range 0..4095"));
    }
    Ok(value)
}

fn extra_data(value: i64) -> PhpResult<u16> {
    u16::try_from(value).map_err(|_| php_error("block extra data must be in range 0..65535"))
}

pub(super) fn revision(value: i64, field: &'static str) -> PhpResult<u64> {
    u64::try_from(value).map_err(|_| php_error(format!("{field} must be nonnegative")))
}

fn resolve_world_state(handle_value: i64) -> PhpResult<Arc<NativeWorldState>> {
    let owner = current_runtime_id().map_err(php_error)?;
    let handle = handle(handle_value)?;
    let arena = world_arena();
    let world = arena
        .get(handle)
        .ok_or_else(|| php_error("native world handle is stale or unknown"))?;
    if world.owner != owner {
        return Err(php_error("native world belongs to another PHP runtime"));
    }
    Ok(Arc::clone(&world.state))
}

pub(crate) fn resolve_world(handle_value: i64) -> PhpResult<Arc<WorldStore>> {
    Ok(Arc::clone(&resolve_world_state(handle_value)?.store))
}

pub(crate) fn protocol84_chunk(handle_value: i64, position: ChunkCoord) -> PhpResult<RawPacket> {
    let state = resolve_world_state(handle_value)?;
    let snapshot = state
        .store
        .snapshot(position)
        .map_err(|error| php_error(error.to_string()))?;

    {
        let cache = match state.protocol84_cache.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(cached) = cache.get(&position)
            && cached.terrain_revision == snapshot.terrain_revision()
            && cached.light_revision == snapshot.light_revision()
        {
            return Ok(cached.packet.clone());
        }
    }

    let mut block_ids = Vec::with_capacity(snapshot.states().len());
    let mut block_data = vec![0_u8; CHUNK_NIBBLE_BYTES];
    for (index, &state_id) in snapshot.states().iter().enumerate() {
        block_ids.push((state_id >> 4) as u8);
        let data = (state_id & 0x0f) as u8;
        let byte = &mut block_data[index >> 1];
        if index & 1 == 0 {
            *byte = (*byte & 0xf0) | data;
        } else {
            *byte = (*byte & 0x0f) | (data << 4);
        }
    }

    let extra_data = snapshot
        .extra_data()
        .iter()
        .map(|(&key, &value)| (u32::from(key), value))
        .collect::<Vec<_>>();
    let packet = encode_protocol84_full_chunk_data(Protocol84ChunkSnapshot {
        chunk_x: position.x(),
        chunk_z: position.z(),
        block_ids: &block_ids,
        block_data: &block_data,
        sky_light: snapshot.sky_light(),
        block_light: snapshot.block_light(),
        biomes: snapshot.biomes(),
        height_map: snapshot.height_map(),
        extra_data: &extra_data,
    })
    .map_err(|error| php_error(error.to_string()))?;

    let current_terrain = state
        .store
        .terrain_revision(position)
        .map_err(|error| php_error(error.to_string()))?;
    let current_light = state
        .store
        .light_revision(position)
        .map_err(|error| php_error(error.to_string()))?;
    if current_terrain == snapshot.terrain_revision() && current_light == snapshot.light_revision()
    {
        let mut cache = match state.protocol84_cache.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if cache.len() >= MAX_PROTOCOL84_CACHE_ENTRIES && !cache.contains_key(&position) {
            cache.clear();
        }
        cache.insert(
            position,
            CachedProtocol84Chunk {
                terrain_revision: snapshot.terrain_revision(),
                light_revision: snapshot.light_revision(),
                packet: packet.clone(),
            },
        );
    }

    Ok(packet)
}

pub(crate) fn register(module: ModuleBuilder) -> ModuleBuilder {
    registration::register(module)
}

pub(crate) fn shutdown() {
    lifecycle::shutdown();
}
