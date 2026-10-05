use std::path::Path;
use std::sync::{Arc, Mutex};

use cobblestone_storage::{read_generator_state, write_generator_state};
use cobblestone_world::{
    ChunkCoord, OverworldInfiniteGenerator, OverworldInfiniteState,
};
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::{php_boundary, php_error};

use super::{NativeWorldState, position, resolve_world_state, storage};

pub(super) const INFINITE_GENERATOR_ID: u32 = 1;

pub(super) struct NativeInfiniteGenerator {
    generator: OverworldInfiniteGenerator,
    dirty: bool,
}

impl NativeInfiniteGenerator {
    fn new(seed: i32, state: OverworldInfiniteState) -> Self {
        Self {
            generator: OverworldInfiniteGenerator::with_state(seed, state),
            dirty: false,
        }
    }
}

pub(super) fn restore_persistent(
    state: &Arc<NativeWorldState>,
    root: &Path,
    world_uuid: [u8; 16],
    generator_id: u32,
    seed: i64,
) -> PhpResult<()> {
    if generator_id != INFINITE_GENERATOR_ID {
        return Ok(());
    }
    let seed = i32::try_from(seed)
        .map_err(|_| php_error("Infinite world seed must fit signed 32 bits"))?;
    let restored = read_generator_state(root, world_uuid, generator_id)
        .map_err(|error| php_error(error.to_string()))?
        .map(|bytes| {
            OverworldInfiniteState::decode(&bytes)
                .ok_or_else(|| php_error("persistent Infinite generator state is invalid"))
        })
        .transpose()?
        .unwrap_or_default();

    let mut infinite = lock_infinite(&state.infinite);
    *infinite = Some(NativeInfiniteGenerator::new(seed, restored));
    Ok(())
}

pub(super) fn persist_if_dirty(
    state: &Arc<NativeWorldState>,
    root: &Path,
    world_uuid: [u8; 16],
    generator_id: u32,
) -> PhpResult<()> {
    if generator_id != INFINITE_GENERATOR_ID {
        return Ok(());
    }

    let mut infinite = lock_infinite(&state.infinite);
    let Some(runtime) = infinite.as_mut() else {
        return Ok(());
    };
    if !runtime.dirty {
        return Ok(());
    }

    let encoded = runtime.generator.state().encode();
    write_generator_state(root, world_uuid, generator_id, &encoded)
        .map_err(|error| php_error(error.to_string()))?;
    runtime.dirty = false;
    Ok(())
}

fn ensure_runtime(state: &Arc<NativeWorldState>, seed: i32) -> PhpResult<()> {
    let mut infinite = lock_infinite(&state.infinite);
    match infinite.as_ref() {
        Some(runtime) if runtime.generator.seed() == seed => Ok(()),
        Some(_) => Err(php_error(
            "native Infinite generator is already configured for a different seed",
        )),
        None => {
            *infinite = Some(NativeInfiniteGenerator::new(
                seed,
                OverworldInfiniteState::new(),
            ));
            Ok(())
        }
    }
}

fn neighborhood(center: ChunkCoord) -> [ChunkCoord; 9] {
    std::array::from_fn(|index| {
        let offset_x = (index % 3) as i32 - 1;
        let offset_z = (index / 3) as i32 - 1;
        ChunkCoord::new(
            center.x().wrapping_add(offset_x),
            center.z().wrapping_add(offset_z),
        )
    })
}

fn lock_infinite(
    mutex: &Mutex<Option<NativeInfiniteGenerator>>,
) -> std::sync::MutexGuard<'_, Option<NativeInfiniteGenerator>> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Returns 0 when the center is resident/complete and 1 while storage dependencies are loading.
#[php_function]
pub fn cobblestone_world_generate_infinite(
    handle_value: i64,
    seed: i64,
    chunk_x: i64,
    chunk_z: i64,
) -> PhpResult<i64> {
    php_boundary(|| {
        let seed =
            i32::try_from(seed).map_err(|_| php_error("Infinite world seed must fit signed 32 bits"))?;
        let target = position(chunk_x, chunk_z)?;
        let state = resolve_world_state(handle_value)?;
        let required = neighborhood(target);

        if !storage::prepare_generation_neighborhood(&state, &required)? {
            return Ok(1);
        }
        ensure_runtime(&state, seed)?;

        let result = {
            let mut infinite = lock_infinite(&state.infinite);
            let runtime = infinite
                .as_mut()
                .expect("Infinite runtime initialized immediately above");
            let result = runtime
                .generator
                .generate_into_store(&state.store, target)
                .map_err(|error| php_error(error.to_string()))?;
            if result.generated_center() {
                runtime.dirty = true;
            }
            result
        };

        if result.generated_center() {
            storage::clear_missing_after_generation(&state, &required);
        }
        Ok(0)
    })
}

pub(super) fn register(module: ModuleBuilder) -> ModuleBuilder {
    module.function(wrap_function!(cobblestone_world_generate_infinite))
}
