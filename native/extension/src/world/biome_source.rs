use cobblestone_world::OverworldBiomeSource;
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
}
