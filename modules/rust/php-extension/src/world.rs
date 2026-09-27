use std::sync::{Arc, Mutex, MutexGuard};

use cobblestone_core::{
    Arena, ChunkCoord, ChunkPatch as NativeChunkPatch, Handle, RuntimeId, WorldStore,
    CHUNK_NIBBLE_BYTES,
};
use ext_php_rs::binary::Binary;
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::{php_boundary, php_error};
use crate::runtime::current_runtime_id;

static WORLD_ARENA: Mutex<Arena<NativeWorld>> = Mutex::new(Arena::new());

struct NativeWorld {
    owner: RuntimeId,
    store: Arc<WorldStore>,
}

fn world_arena() -> MutexGuard<'static, Arena<NativeWorld>> {
    match WORLD_ARENA.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn handle(value: i64) -> PhpResult<Handle<NativeWorld>> {
    Handle::from_raw(value as u64).ok_or_else(|| php_error("invalid native world handle"))
}

fn position(x: i64, z: i64) -> PhpResult<ChunkCoord> {
    let x = i32::try_from(x).map_err(|_| php_error("chunk x must fit signed 32 bits"))?;
    let z = i32::try_from(z).map_err(|_| php_error("chunk z must fit signed 32 bits"))?;
    Ok(ChunkCoord::new(x, z))
}

fn byte(value: i64, field: &'static str) -> PhpResult<u8> {
    u8::try_from(value).map_err(|_| php_error(format!("{field} must fit one byte")))
}

fn local(value: i64, field: &'static str) -> PhpResult<u8> {
    let value = byte(value, field)?;
    if value > 15 {
        return Err(php_error(format!("{field} must be in range 0..15")));
    }
    Ok(value)
}

fn block_y(value: i64) -> PhpResult<u8> {
    let value = byte(value, "block y")?;
    if value > 127 {
        return Err(php_error("block y must be in range 0..127"));
    }
    Ok(value)
}

fn fill_y(value: i64) -> PhpResult<u8> {
    let value = byte(value, "fill y")?;
    if value > 128 {
        return Err(php_error("fill y must be in range 0..128"));
    }
    Ok(value)
}

fn state_id(value: i64) -> PhpResult<u16> {
    let value = u16::try_from(value).map_err(|_| php_error("block state id must fit 16 bits"))?;
    if value > 0x0fff {
        return Err(php_error("block state id must be in range 0..4095"));
    }
    Ok(value)
}

fn extra_data(value: i64) -> PhpResult<u16> {
    u16::try_from(value).map_err(|_| php_error("block extra data must be in range 0..65535"))
}

fn revision(value: i64, field: &'static str) -> PhpResult<u64> {
    u64::try_from(value).map_err(|_| php_error(format!("{field} must be nonnegative")))
}

fn php_revision(value: u64) -> PhpResult<i64> {
    i64::try_from(value).map_err(|_| php_error("native world revision exceeds PHP integer range"))
}


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
        blocks.push((
            reader.u16("block index")?,
            reader.u16("block state")?,
        ));
    }

    let mut biomes = Vec::new();
    for _ in 0..biome_count {
        biomes.push((
            reader.u8("biome index")?,
            reader.u8("biome value")?,
        ));
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

pub(crate) fn resolve_world(handle_value: i64) -> PhpResult<Arc<WorldStore>> {
    let owner = current_runtime_id().map_err(php_error)?;
    let handle = handle(handle_value)?;
    let arena = world_arena();
    let world = arena
        .get(handle)
        .ok_or_else(|| php_error("native world handle is stale or unknown"))?;
    if world.owner != owner {
        return Err(php_error("native world belongs to another PHP runtime"));
    }
    Ok(Arc::clone(&world.store))
}

#[php_function]
pub fn cobblestone_world_create() -> PhpResult<i64> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let handle = world_arena()
            .insert(NativeWorld {
                owner,
                store: Arc::new(WorldStore::new()),
            })
            .map_err(|_| php_error("native world handle capacity exhausted"))?;
        Ok(handle.into_raw() as i64)
    })
}

#[php_function]
pub fn cobblestone_world_destroy(handle_value: i64) -> PhpResult<()> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let handle = handle(handle_value)?;
        let mut arena = world_arena();
        let world = arena
            .get(handle)
            .ok_or_else(|| php_error("native world handle is stale or unknown"))?;
        if world.owner != owner {
            return Err(php_error("native world belongs to another PHP runtime"));
        }
        arena
            .remove(handle)
            .ok_or_else(|| php_error("native world handle disappeared"))?;
        Ok(())
    })
}

#[php_function]
pub fn cobblestone_world_ensure_chunk(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
    biome: i64,
) -> PhpResult<bool> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        Ok(store.ensure_chunk(position(chunk_x, chunk_z)?, byte(biome, "biome")?))
    })
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
                .biome(position(chunk_x, chunk_z)?, local(x, "local x")?, local(z, "local z")?)
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
                .height_map(position(chunk_x, chunk_z)?, local(x, "local x")?, local(z, "local z")?)
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
            16
                + snapshot.states().len()
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

pub(crate) fn register(module: ModuleBuilder) -> ModuleBuilder {
    module
        .function(wrap_function!(cobblestone_world_create))
        .function(wrap_function!(cobblestone_world_destroy))
        .function(wrap_function!(cobblestone_world_ensure_chunk))
        .function(wrap_function!(cobblestone_world_terrain_revision))
        .function(wrap_function!(cobblestone_world_light_revision))
        .function(wrap_function!(cobblestone_world_commit_terrain_revision))
        .function(wrap_function!(cobblestone_world_commit_light_revision))
        .function(wrap_function!(cobblestone_world_block_state))
        .function(wrap_function!(cobblestone_world_set_block_state))
        .function(wrap_function!(cobblestone_world_fill_layers))
        .function(wrap_function!(cobblestone_world_biome))
        .function(wrap_function!(cobblestone_world_set_biome))
        .function(wrap_function!(cobblestone_world_fill_biome))
        .function(wrap_function!(cobblestone_world_sky_light))
        .function(wrap_function!(cobblestone_world_set_sky_light))
        .function(wrap_function!(cobblestone_world_fill_sky_light_from))
        .function(wrap_function!(cobblestone_world_block_light))
        .function(wrap_function!(cobblestone_world_set_block_light))
        .function(wrap_function!(cobblestone_world_height_map))
        .function(wrap_function!(cobblestone_world_recalculate_height_map))
        .function(wrap_function!(cobblestone_world_block_extra_data))
        .function(wrap_function!(cobblestone_world_set_block_extra_data))
        .function(wrap_function!(cobblestone_world_apply_patch))
        .function(wrap_function!(cobblestone_world_snapshot))
}

pub(crate) fn shutdown() {
    *world_arena() = Arena::new();
}
