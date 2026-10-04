mod block;
mod block_extra;
mod height;
mod lifecycle;
mod light;
mod patch;
mod protocol84;
mod registration;
mod residency;
mod revision;
mod snapshot;
mod storage;

use std::sync::{Arc, Mutex, MutexGuard};

use cobblestone_runtime::{Arena, Handle, RuntimeId};
use cobblestone_world::{ChunkCoord, WorldStore, block_state_id_is_supported};
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::php_error;
use crate::runtime::current_runtime_id;

static WORLD_ARENA: Mutex<Arena<NativeWorld>> = Mutex::new(Arena::new());

struct NativeWorld {
    owner: RuntimeId,
    state: Arc<NativeWorldState>,
}

struct NativeWorldState {
    store: Arc<WorldStore>,
    protocol84_cache: Mutex<protocol84::Protocol84Cache>,
    persistence: Mutex<Option<storage::NativeWorldPersistence>>,
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

fn word32(value: i64, field: &'static str) -> PhpResult<u32> {
    u32::try_from(value).map_err(|_| php_error(format!("{field} must fit unsigned 32 bits")))
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
    if !block_state_id_is_supported(value) {
        return Err(php_error(format!(
            "unsupported MCPE 0.15.10 block state id {value}"
        )));
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

pub(crate) use protocol84::protocol84_chunk;

pub(crate) fn register(module: ModuleBuilder) -> ModuleBuilder {
    registration::register(module)
}

pub(crate) fn shutdown() {
    lifecycle::shutdown();
}
