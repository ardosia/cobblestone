use cobblestone_storage::WorldMetadata;
use cobblestone_world::DimensionId;
use ext_php_rs::binary::Binary;
use ext_php_rs::convert::IntoZval;
use ext_php_rs::exception::PhpResult;
use ext_php_rs::types::{ZendHashTable, Zval};

use crate::boundary::php_error;

pub(super) fn zval<T: IntoZval>(value: T) -> PhpResult<Zval> {
    value
        .into_zval(false)
        .map_err(|error| php_error(error.to_string()))
}

pub(super) fn metadata_values(created: bool, metadata: &WorldMetadata) -> PhpResult<Vec<Zval>> {
    Ok(vec![
        zval(created)?,
        zval(Binary::new(metadata.world_uuid.to_vec()))?,
        zval(metadata.name.clone())?,
        zval(metadata.seed)?,
        zval(i64::from(metadata.generator_id))?,
        zval(i64::from(metadata.generator_settings_version))?,
        zval(Binary::new(metadata.generator_settings.clone()))?,
        zval(i64::from(u8::from(metadata.dimension)))?,
        zval(i64::from(metadata.spawn_x))?,
        zval(i64::from(metadata.spawn_y))?,
        zval(i64::from(metadata.spawn_z))?,
        zval(metadata.time)?,
        zval(metadata.time_running)?,
        zval(
            i64::try_from(metadata.generation)
                .map_err(|_| php_error("world metadata generation exceeds PHP integer range"))?,
        )?,
    ])
}

fn creation_value<'a>(
    values: &'a ZendHashTable,
    index: i64,
    field: &'static str,
) -> PhpResult<&'a Zval> {
    values.get_index(index).ok_or_else(|| {
        php_error(format!(
            "world storage creation metadata is missing {field}"
        ))
    })
}

fn creation_long(values: &ZendHashTable, index: i64, field: &'static str) -> PhpResult<i64> {
    creation_value(values, index, field)?
        .long()
        .ok_or_else(|| php_error(format!("world storage creation {field} must be an integer")))
}

pub(super) fn parse_creation_metadata(values: &ZendHashTable) -> PhpResult<WorldMetadata> {
    if values.len() != 12 {
        return Err(php_error(
            "world storage creation metadata must contain exactly 12 values",
        ));
    }

    let uuid = creation_value(values, 0, "UUID")?
        .binary::<u8>()
        .ok_or_else(|| php_error("world storage creation UUID must be binary"))?;
    let world_uuid: [u8; 16] = uuid
        .as_slice()
        .try_into()
        .map_err(|_| php_error("world UUID must contain exactly 16 bytes"))?;
    let name = creation_value(values, 1, "name")?
        .string()
        .ok_or_else(|| php_error("world storage creation name must be a string"))?;
    let seed = creation_long(values, 2, "seed")?;
    let generator_id = u32::try_from(creation_long(values, 3, "generator id")?)
        .map_err(|_| php_error("generator id must fit unsigned 32 bits"))?;
    let generator_settings_version =
        u16::try_from(creation_long(values, 4, "generator settings version")?)
            .map_err(|_| php_error("generator settings version must fit unsigned 16 bits"))?;
    let generator_settings = creation_value(values, 5, "generator settings")?
        .binary::<u8>()
        .ok_or_else(|| php_error("world storage creation generator settings must be binary"))?;

    let mut metadata = WorldMetadata::new(
        world_uuid,
        name,
        seed,
        generator_id,
        generator_settings_version,
        generator_settings,
        DimensionId::try_from(
            u8::try_from(creation_long(values, 11, "dimension id")?)
                .map_err(|_| php_error("dimension id must fit unsigned 8 bits"))?,
        )
        .map_err(|_| php_error("unsupported MCPE 0.15.10 dimension id"))?,
    );
    metadata.spawn_x = i32::try_from(creation_long(values, 6, "spawn x")?)
        .map_err(|_| php_error("spawn x must fit signed 32 bits"))?;
    metadata.spawn_y = i32::try_from(creation_long(values, 7, "spawn y")?)
        .map_err(|_| php_error("spawn y must fit signed 32 bits"))?;
    metadata.spawn_z = i32::try_from(creation_long(values, 8, "spawn z")?)
        .map_err(|_| php_error("spawn z must fit signed 32 bits"))?;
    metadata.time = creation_long(values, 9, "time")?;
    metadata.time_running = creation_value(values, 10, "time-running flag")?
        .bool()
        .ok_or_else(|| php_error("world storage creation time-running flag must be boolean"))?;

    Ok(metadata)
}
