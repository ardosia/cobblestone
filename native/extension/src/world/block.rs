use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::{php_boundary, php_error};

use super::{block_y, byte, local, position, resolve_world, state_id};

#[php_function]
pub fn cobblestone_world_block_state(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
    x: i64,
    y: i64,
    z: i64,
) -> PhpResult<i64> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        Ok(i64::from(
            store
                .block_state(
                    position(chunk_x, chunk_z)?,
                    local(x, "local x")?,
                    block_y(y)?,
                    local(z, "local z")?,
                )
                .map_err(|error| php_error(error.to_string()))?,
        ))
    })
}

#[php_function]
pub fn cobblestone_world_set_block_state(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
    x: i64,
    y: i64,
    z: i64,
    state: i64,
) -> PhpResult<i64> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        Ok(i64::from(
            store
                .set_block_state(
                    position(chunk_x, chunk_z)?,
                    local(x, "local x")?,
                    block_y(y)?,
                    local(z, "local z")?,
                    state_id(state)?,
                )
                .map_err(|error| php_error(error.to_string()))?,
        ))
    })
}

#[php_function]
pub fn cobblestone_world_fill_layers(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
    start_y: i64,
    count: i64,
    state: i64,
) -> PhpResult<()> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        store
            .fill_layers(
                position(chunk_x, chunk_z)?,
                block_y(start_y)?,
                byte(count, "layer count")?,
                state_id(state)?,
            )
            .map_err(|error| php_error(error.to_string()))
    })
}

#[php_function]
pub fn cobblestone_world_biome(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
    x: i64,
    z: i64,
) -> PhpResult<i64> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        Ok(i64::from(
            store
                .biome(
                    position(chunk_x, chunk_z)?,
                    local(x, "local x")?,
                    local(z, "local z")?,
                )
                .map_err(|error| php_error(error.to_string()))?,
        ))
    })
}

#[php_function]
pub fn cobblestone_world_set_biome(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
    x: i64,
    z: i64,
    biome: i64,
) -> PhpResult<i64> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        Ok(i64::from(
            store
                .set_biome(
                    position(chunk_x, chunk_z)?,
                    local(x, "local x")?,
                    local(z, "local z")?,
                    byte(biome, "biome")?,
                )
                .map_err(|error| php_error(error.to_string()))?,
        ))
    })
}

#[php_function]
pub fn cobblestone_world_fill_biome(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
    biome: i64,
) -> PhpResult<()> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        store
            .fill_biome(position(chunk_x, chunk_z)?, byte(biome, "biome")?)
            .map_err(|error| php_error(error.to_string()))
    })
}

pub(super) fn register(module: ModuleBuilder) -> ModuleBuilder {
    module
        .function(wrap_function!(cobblestone_world_block_state))
        .function(wrap_function!(cobblestone_world_set_block_state))
        .function(wrap_function!(cobblestone_world_fill_layers))
        .function(wrap_function!(cobblestone_world_biome))
        .function(wrap_function!(cobblestone_world_set_biome))
        .function(wrap_function!(cobblestone_world_fill_biome))
}
