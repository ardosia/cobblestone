use cobblestone_world::CHUNK_NIBBLE_BYTES;
use ext_php_rs::binary::Binary;
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::{php_boundary, php_error};

use super::{position, resolve_world};

#[php_function]
pub fn cobblestone_world_snapshot(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
) -> PhpResult<Binary<u8>> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        let snapshot = store
            .snapshot(position(chunk_x, chunk_z)?)
            .map_err(|error| php_error(error.to_string()))?;

        let extra_count = u32::try_from(snapshot.extra_data().len())
            .map_err(|_| php_error("native chunk extra-data entry count exceeds u32"))?;
        let mut projection = Vec::with_capacity(
            16 + snapshot.states().len()
                + CHUNK_NIBBLE_BYTES
                + snapshot.sky_light().len()
                + snapshot.block_light().len()
                + snapshot.biomes().len()
                + snapshot.height_map().len()
                + 4
                + snapshot.extra_data().len() * 4,
        );

        projection.extend_from_slice(&snapshot.terrain_revision().to_le_bytes());
        projection.extend_from_slice(&snapshot.light_revision().to_le_bytes());

        for &state in snapshot.states() {
            projection.push((state >> 4) as u8);
        }

        let mut block_data = vec![0_u8; CHUNK_NIBBLE_BYTES];
        for (index, &state) in snapshot.states().iter().enumerate() {
            let data = (state & 0x0f) as u8;
            let byte = &mut block_data[index >> 1];
            if index & 1 == 0 {
                *byte = (*byte & 0xf0) | data;
            } else {
                *byte = (*byte & 0x0f) | (data << 4);
            }
        }
        projection.extend_from_slice(&block_data);
        projection.extend_from_slice(snapshot.sky_light());
        projection.extend_from_slice(snapshot.block_light());
        projection.extend_from_slice(snapshot.biomes());
        projection.extend_from_slice(snapshot.height_map());
        projection.extend_from_slice(&extra_count.to_le_bytes());
        for (&key, &value) in snapshot.extra_data() {
            projection.extend_from_slice(&key.to_le_bytes());
            projection.extend_from_slice(&value.to_le_bytes());
        }

        Ok(Binary::new(projection))
    })
}

pub(super) fn register(module: ModuleBuilder) -> ModuleBuilder {
    module.function(wrap_function!(cobblestone_world_snapshot))
}
