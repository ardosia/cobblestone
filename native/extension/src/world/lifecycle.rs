use std::sync::{Arc, Mutex};

use cobblestone_world::WorldStore;
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::{php_boundary, php_error};
use crate::runtime::current_runtime_id;

use super::{NativeWorld, NativeWorldState, handle, storage, wire, world_arena};

#[php_function]
pub fn cobblestone_world_create() -> PhpResult<i64> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let handle = world_arena()
            .insert(NativeWorld {
                owner,
                state: Arc::new(NativeWorldState {
                    store: Arc::new(WorldStore::new()),
                    chunk_wire_cache: Mutex::new(wire::ChunkWireCache::default()),
                    persistence: Mutex::new(None),
                    infinite: Mutex::new(None),
                }),
            })
            .map_err(|_| php_error("native world handle capacity exhausted"))?;
        Ok(handle.into_raw() as i64)
    })
}

#[php_function]
pub fn cobblestone_world_destroy(handle_value: i64) -> PhpResult<()> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let handle = handle(handle_value)?;
        let state = {
            let arena = world_arena();
            let world = arena
                .get(handle)
                .ok_or_else(|| php_error("native world handle is stale or unknown"))?;
            if world.owner != owner {
                return Err(php_error("native world belongs to another PHP runtime"));
            }
            let pins = world.state.store.total_pin_count();
            if pins != 0 {
                return Err(php_error(format!(
                    "native world cannot be destroyed while {pins} chunk pins are active"
                )));
            }
            Arc::clone(&world.state)
        };

        {
            let mut persistence = match state.persistence.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if let Some(persistence) = persistence.as_mut() {
                storage::flush_all(&state, persistence)?;
            }
        }

        let mut arena = world_arena();
        let world = arena
            .get(handle)
            .ok_or_else(|| php_error("native world handle disappeared during shutdown"))?;
        if world.owner != owner {
            return Err(php_error("native world owner changed during shutdown"));
        }
        arena
            .remove(handle)
            .ok_or_else(|| php_error("native world handle disappeared"))?;
        Ok(())
    })
}

pub(super) fn shutdown() {
    let worlds = world_arena().drain();

    for world in worlds {
        let mut persistence = match world.state.persistence.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(persistence) = persistence.as_mut() {
            let _ = storage::flush_all(&world.state, persistence);
        }
    }
}

pub(super) fn register(module: ModuleBuilder) -> ModuleBuilder {
    module
        .function(wrap_function!(cobblestone_world_create))
        .function(wrap_function!(cobblestone_world_destroy))
}
