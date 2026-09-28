use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::{php_boundary, php_error};

use super::{block_y, byte, fill_y, local, position, resolve_world};

#[php_function]
pub fn cobblestone_world_sky_light(
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
                .sky_light(
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
pub fn cobblestone_world_set_sky_light(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
    x: i64,
    y: i64,
    z: i64,
    level: i64,
) -> PhpResult<i64> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        Ok(i64::from(
            store
                .set_sky_light(
                    position(chunk_x, chunk_z)?,
                    local(x, "local x")?,
                    block_y(y)?,
                    local(z, "local z")?,
                    byte(level, "sky light")?,
                )
                .map_err(|error| php_error(error.to_string()))?,
        ))
    })
}

#[php_function]
pub fn cobblestone_world_fill_sky_light_from(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
    y: i64,
    level: i64,
) -> PhpResult<()> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        store
            .fill_sky_light_from(
                position(chunk_x, chunk_z)?,
                fill_y(y)?,
                byte(level, "sky light")?,
            )
            .map_err(|error| php_error(error.to_string()))
    })
}

#[php_function]
pub fn cobblestone_world_block_light(
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
                .block_light(
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
pub fn cobblestone_world_set_block_light(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
    x: i64,
    y: i64,
    z: i64,
    level: i64,
) -> PhpResult<i64> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        Ok(i64::from(
            store
                .set_block_light(
                    position(chunk_x, chunk_z)?,
                    local(x, "local x")?,
                    block_y(y)?,
                    local(z, "local z")?,
                    byte(level, "block light")?,
                )
                .map_err(|error| php_error(error.to_string()))?,
        ))
    })
}

pub(super) fn register(module: ModuleBuilder) -> ModuleBuilder {
    module
        .function(wrap_function!(cobblestone_world_sky_light))
        .function(wrap_function!(cobblestone_world_set_sky_light))
        .function(wrap_function!(cobblestone_world_fill_sky_light_from))
        .function(wrap_function!(cobblestone_world_block_light))
        .function(wrap_function!(cobblestone_world_set_block_light))
}
