use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::{php_boundary, php_error};

use super::{local, position, resolve_world};

#[php_function]
pub fn cobblestone_world_height_map(
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
                .height_map(
                    position(chunk_x, chunk_z)?,
                    local(x, "local x")?,
                    local(z, "local z")?,
                )
                .map_err(|error| php_error(error.to_string()))?,
        ))
    })
}

#[php_function]
pub fn cobblestone_world_recalculate_height_map(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
) -> PhpResult<()> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        store
            .recalculate_height_map(position(chunk_x, chunk_z)?)
            .map_err(|error| php_error(error.to_string()))
    })
}

pub(super) fn register(module: ModuleBuilder) -> ModuleBuilder {
    module
        .function(wrap_function!(cobblestone_world_height_map))
        .function(wrap_function!(cobblestone_world_recalculate_height_map))
}
