use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::{php_boundary, php_error};

use super::{block_y, extra_data, local, position, resolve_world};

#[php_function]
pub fn cobblestone_world_block_extra_data(
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
                .block_extra_data(
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
pub fn cobblestone_world_set_block_extra_data(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
    x: i64,
    y: i64,
    z: i64,
    value: i64,
) -> PhpResult<i64> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        Ok(i64::from(
            store
                .set_block_extra_data(
                    position(chunk_x, chunk_z)?,
                    local(x, "local x")?,
                    block_y(y)?,
                    local(z, "local z")?,
                    extra_data(value)?,
                )
                .map_err(|error| php_error(error.to_string()))?,
        ))
    })
}

pub(super) fn register(module: ModuleBuilder) -> ModuleBuilder {
    module
        .function(wrap_function!(cobblestone_world_block_extra_data))
        .function(wrap_function!(cobblestone_world_set_block_extra_data))
}
