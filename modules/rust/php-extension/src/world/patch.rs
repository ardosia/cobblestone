use cobblestone_core::ChunkPatch as NativeChunkPatch;
use ext_php_rs::binary::Binary;
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::{php_boundary, php_error};

use super::{position, resolve_world};

struct PatchReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> PatchReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take<const N: usize>(&mut self, field: &'static str) -> PhpResult<[u8; N]> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or_else(|| php_error("native world patch offset overflow"))?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| php_error(format!("native world patch is truncated at {field}")))?;
        self.offset = end;
        slice
            .try_into()
            .map_err(|_| php_error(format!("native world patch has invalid {field} width")))
    }

    fn u8(&mut self, field: &'static str) -> PhpResult<u8> {
        Ok(self.take::<1>(field)?[0])
    }

    fn u16(&mut self, field: &'static str) -> PhpResult<u16> {
        Ok(u16::from_le_bytes(self.take::<2>(field)?))
    }

    fn u32(&mut self, field: &'static str) -> PhpResult<u32> {
        Ok(u32::from_le_bytes(self.take::<4>(field)?))
    }

    fn u64(&mut self, field: &'static str) -> PhpResult<u64> {
        Ok(u64::from_le_bytes(self.take::<8>(field)?))
    }

    fn finish(self) -> PhpResult<()> {
        if self.offset != self.bytes.len() {
            return Err(php_error("native world patch has trailing bytes"));
        }
        Ok(())
    }
}
fn parse_patch(bytes: &[u8]) -> PhpResult<NativeChunkPatch> {
    let mut reader = PatchReader::new(bytes);
    let expected_terrain_revision = reader.u64("expected terrain revision")?;
    let next_terrain_revision = reader.u64("next terrain revision")?;
    let expected_light_revision = reader.u64("expected light revision")?;
    let next_light_revision = reader.u64("next light revision")?;

    let block_count = usize::try_from(reader.u32("block count")?)
        .map_err(|_| php_error("native world patch block count exceeds platform size"))?;
    let biome_count = usize::try_from(reader.u32("biome count")?)
        .map_err(|_| php_error("native world patch biome count exceeds platform size"))?;
    let extra_count = usize::try_from(reader.u32("extra-data count")?)
        .map_err(|_| php_error("native world patch extra-data count exceeds platform size"))?;
    let sky_count = usize::try_from(reader.u32("sky-light count")?)
        .map_err(|_| php_error("native world patch sky-light count exceeds platform size"))?;
    let block_light_count = usize::try_from(reader.u32("block-light count")?)
        .map_err(|_| php_error("native world patch block-light count exceeds platform size"))?;

    let mut blocks = Vec::new();
    for _ in 0..block_count {
        blocks.push((reader.u16("block index")?, reader.u16("block state")?));
    }

    let mut biomes = Vec::new();
    for _ in 0..biome_count {
        biomes.push((reader.u8("biome index")?, reader.u8("biome value")?));
    }

    let mut extra_data = Vec::new();
    for _ in 0..extra_count {
        extra_data.push((
            reader.u16("extra-data index")?,
            reader.u16("extra-data value")?,
        ));
    }

    let mut sky_light = Vec::new();
    for _ in 0..sky_count {
        sky_light.push((
            reader.u16("sky-light index")?,
            reader.u8("sky-light value")?,
        ));
    }

    let mut block_light = Vec::new();
    for _ in 0..block_light_count {
        block_light.push((
            reader.u16("block-light index")?,
            reader.u8("block-light value")?,
        ));
    }
    reader.finish()?;
    Ok(NativeChunkPatch {
        expected_terrain_revision,
        next_terrain_revision,
        expected_light_revision,
        next_light_revision,
        blocks,
        biomes,
        extra_data,
        sky_light,
        block_light,
    })
}

#[php_function]
pub fn cobblestone_world_apply_patch(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
    patch: Binary<u8>,
) -> PhpResult<()> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        let bytes: Vec<u8> = patch.into();
        let patch = parse_patch(&bytes)?;
        store
            .apply_patch(position(chunk_x, chunk_z)?, patch)
            .map_err(|error| php_error(error.to_string()))
    })
}
pub(super) fn register(module: ModuleBuilder) -> ModuleBuilder {
    module.function(wrap_function!(cobblestone_world_apply_patch))
}
