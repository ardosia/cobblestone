use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::{php_boundary, php_error};

use super::{position, resolve_world, revision};

fn php_revision(value: u64) -> PhpResult<i64> {
    i64::try_from(value).map_err(|_| php_error("native world revision exceeds PHP integer range"))
}

#[php_function]
pub fn cobblestone_world_terrain_revision(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
) -> PhpResult<i64> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        php_revision(
            store
                .terrain_revision(position(chunk_x, chunk_z)?)
                .map_err(|error| php_error(error.to_string()))?,
        )
    })
}

#[php_function]
pub fn cobblestone_world_light_revision(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
) -> PhpResult<i64> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        php_revision(
            store
                .light_revision(position(chunk_x, chunk_z)?)
                .map_err(|error| php_error(error.to_string()))?,
        )
    })
}

#[php_function]
pub fn cobblestone_world_commit_terrain_revision(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
    expected: i64,
    next: i64,
) -> PhpResult<()> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        store
            .commit_terrain_revision(
                position(chunk_x, chunk_z)?,
                revision(expected, "expected terrain revision")?,
                revision(next, "next terrain revision")?,
            )
            .map_err(|error| php_error(error.to_string()))
    })
}

#[php_function]
pub fn cobblestone_world_commit_light_revision(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
    expected: i64,
    next: i64,
) -> PhpResult<()> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        store
            .commit_light_revision(
                position(chunk_x, chunk_z)?,
                revision(expected, "expected light revision")?,
                revision(next, "next light revision")?,
            )
            .map_err(|error| php_error(error.to_string()))
    })
}

pub(super) fn register(module: ModuleBuilder) -> ModuleBuilder {
    module
        .function(wrap_function!(cobblestone_world_terrain_revision))
        .function(wrap_function!(cobblestone_world_light_revision))
        .function(wrap_function!(cobblestone_world_commit_terrain_revision))
        .function(wrap_function!(cobblestone_world_commit_light_revision))
}
