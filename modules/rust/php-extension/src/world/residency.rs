use cobblestone_core::ChunkEviction;
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::{php_boundary, php_error};

use super::{byte, position, resolve_world, resolve_world_state, revision};

#[php_function]
pub fn cobblestone_world_ensure_chunk(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
    biome: i64,
) -> PhpResult<bool> {
    php_boundary(|| {
        let position = position(chunk_x, chunk_z)?;
        let state = resolve_world_state(handle_value)?;
        let inserted = state.store.ensure_chunk(position, byte(biome, "biome")?);
        if inserted {
            let mut persistence = match state.persistence.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if let Some(persistence) = persistence.as_mut() {
                persistence.clear_missing(position);
            }
        }
        Ok(inserted)
    })
}

#[php_function]
pub fn cobblestone_world_lifecycle_flags(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
) -> PhpResult<i64> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        Ok(i64::from(
            store
                .lifecycle_flags(position(chunk_x, chunk_z)?)
                .map_err(|error| php_error(error.to_string()))?,
        ))
    })
}

#[php_function]
pub fn cobblestone_world_set_lifecycle_flags(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
    flags: i64,
) -> PhpResult<()> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        store
            .set_lifecycle_flags(
                position(chunk_x, chunk_z)?,
                byte(flags, "chunk lifecycle flags")?,
            )
            .map_err(|error| php_error(error.to_string()))
    })
}

#[php_function]
pub fn cobblestone_world_pin_chunk(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
) -> PhpResult<i64> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        Ok(i64::from(
            store
                .pin_chunk(position(chunk_x, chunk_z)?)
                .map_err(|error| php_error(error.to_string()))?,
        ))
    })
}

#[php_function]
pub fn cobblestone_world_unpin_chunk(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
) -> PhpResult<i64> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        Ok(i64::from(
            store
                .unpin_chunk(position(chunk_x, chunk_z)?)
                .map_err(|error| php_error(error.to_string()))?,
        ))
    })
}

#[php_function]
pub fn cobblestone_world_chunk_pin_count(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
) -> PhpResult<i64> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        Ok(i64::from(
            store
                .pin_count(position(chunk_x, chunk_z)?)
                .map_err(|error| php_error(error.to_string()))?,
        ))
    })
}

#[php_function]
pub fn cobblestone_world_chunk_dirty(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
) -> PhpResult<bool> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        store
            .is_dirty(position(chunk_x, chunk_z)?)
            .map_err(|error| php_error(error.to_string()))
    })
}

#[php_function]
pub fn cobblestone_world_mark_persisted(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
    terrain_revision: i64,
    light_revision: i64,
    lifecycle_flags: i64,
) -> PhpResult<()> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        store
            .mark_persisted(
                position(chunk_x, chunk_z)?,
                revision(terrain_revision, "persisted terrain revision")?,
                revision(light_revision, "persisted light revision")?,
                byte(lifecycle_flags, "persisted lifecycle flags")?,
            )
            .map_err(|error| php_error(error.to_string()))
    })
}

/// Attempts a safe residency eviction.
///
/// Return codes are stable at the PHP bridge: 0=missing, 1=pinned, 2=dirty, 3=evicted.
#[php_function]
pub fn cobblestone_world_try_evict_chunk(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
) -> PhpResult<i64> {
    php_boundary(|| {
        let position = position(chunk_x, chunk_z)?;
        let state = resolve_world_state(handle_value)?;
        let result = state
            .store
            .try_evict_chunk(position)
            .map_err(|error| php_error(error.to_string()))?;

        if result == ChunkEviction::Evicted {
            let mut cache = match state.protocol84_cache.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            cache.remove(&position);
        }

        Ok(match result {
            ChunkEviction::Missing => 0,
            ChunkEviction::Pinned { .. } => 1,
            ChunkEviction::Dirty => 2,
            ChunkEviction::Evicted => 3,
        })
    })
}

pub(super) fn register(module: ModuleBuilder) -> ModuleBuilder {
    module
        .function(wrap_function!(cobblestone_world_ensure_chunk))
        .function(wrap_function!(cobblestone_world_lifecycle_flags))
        .function(wrap_function!(cobblestone_world_set_lifecycle_flags))
        .function(wrap_function!(cobblestone_world_pin_chunk))
        .function(wrap_function!(cobblestone_world_unpin_chunk))
        .function(wrap_function!(cobblestone_world_chunk_pin_count))
        .function(wrap_function!(cobblestone_world_chunk_dirty))
        .function(wrap_function!(cobblestone_world_mark_persisted))
        .function(wrap_function!(cobblestone_world_try_evict_chunk))
}
