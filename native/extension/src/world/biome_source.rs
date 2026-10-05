use cobblestone_world::{
    OverworldBiomeSource, resolve_overworld_initial_spawn, resolve_overworld_spawn_from_store,
};

use super::resolve_world;
use ext_php_rs::binary::Binary;
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::{php_boundary, php_error};

const MAX_BIOME_AREA_EDGE: usize = 64;

#[php_function]
pub fn cobblestone_world_overworld_biomes(
    seed: i64,
    x: i64,
    z: i64,
    width: i64,
    height: i64,
) -> PhpResult<Binary<u8>> {
    php_boundary(|| {
        let seed =
            i32::try_from(seed).map_err(|_| php_error("world seed must fit signed 32 bits"))?;
        let x = i32::try_from(x).map_err(|_| php_error("biome x must fit signed 32 bits"))?;
        let z = i32::try_from(z).map_err(|_| php_error("biome z must fit signed 32 bits"))?;
        let width = area_edge(width, "biome area width")?;
        let height = area_edge(height, "biome area height")?;

        area_end(x, width, "biome x")?;
        area_end(z, height, "biome z")?;

        Ok(Binary::new(
            OverworldBiomeSource::new(seed).biome_ids(x, z, width, height),
        ))
    })
}

#[php_function]
pub fn cobblestone_world_overworld_spawn(seed: i64) -> PhpResult<Binary<u8>> {
    php_boundary(|| {
        let seed =
            i32::try_from(seed).map_err(|_| php_error("world seed must fit signed 32 bits"))?;
        let (x, z) = OverworldBiomeSource::new(seed).spawn_position();
        let mut bytes = Vec::with_capacity(8);
        bytes.extend_from_slice(&x.to_le_bytes());
        bytes.extend_from_slice(&z.to_le_bytes());
        Ok(Binary::new(bytes))
    })
}

#[php_function]
pub fn cobblestone_world_overworld_initial_spawn(seed: i64) -> PhpResult<Binary<u8>> {
    php_boundary(|| {
        let seed =
            i32::try_from(seed).map_err(|_| php_error("world seed must fit signed 32 bits"))?;
        let [x, y, z] =
            resolve_overworld_initial_spawn(seed).map_err(|error| php_error(error.to_string()))?;
        Ok(spawn_projection(x, y, z))
    })
}

#[php_function]
pub fn cobblestone_world_resolve_overworld_spawn(
    handle_value: i64,
    spawn_x: i64,
    spawn_z: i64,
) -> PhpResult<Binary<u8>> {
    php_boundary(|| {
        let spawn_x =
            i32::try_from(spawn_x).map_err(|_| php_error("spawn x must fit signed 32 bits"))?;
        let spawn_z =
            i32::try_from(spawn_z).map_err(|_| php_error("spawn z must fit signed 32 bits"))?;
        let store = resolve_world(handle_value)?;
        let [x, y, z] = resolve_overworld_spawn_from_store(&store, spawn_x, spawn_z)
            .map_err(|error| php_error(error.to_string()))?;
        Ok(spawn_projection(x, y, z))
    })
}

fn spawn_projection(x: i32, y: i32, z: i32) -> Binary<u8> {
    let mut bytes = Vec::with_capacity(12);
    bytes.extend_from_slice(&x.to_le_bytes());
    bytes.extend_from_slice(&y.to_le_bytes());
    bytes.extend_from_slice(&z.to_le_bytes());
    Binary::new(bytes)
}

fn area_edge(value: i64, field: &'static str) -> PhpResult<usize> {
    let value =
        usize::try_from(value).map_err(|_| php_error(format!("{field} must be positive")))?;
    if value == 0 || value > MAX_BIOME_AREA_EDGE {
        return Err(php_error(format!(
            "{field} must be in range 1..{MAX_BIOME_AREA_EDGE}"
        )));
    }
    Ok(value)
}

fn area_end(origin: i32, length: usize, field: &'static str) -> PhpResult<i32> {
    let offset = i32::try_from(length - 1).expect("bounded biome edge fits i32");
    origin
        .checked_add(offset)
        .ok_or_else(|| php_error(format!("{field} area exceeds signed 32-bit coordinates")))
}

pub(super) fn register(module: ModuleBuilder) -> ModuleBuilder {
    module
        .function(wrap_function!(cobblestone_world_overworld_biomes))
        .function(wrap_function!(cobblestone_world_overworld_spawn))
        .function(wrap_function!(cobblestone_world_overworld_initial_spawn))
        .function(wrap_function!(cobblestone_world_resolve_overworld_spawn))
}
