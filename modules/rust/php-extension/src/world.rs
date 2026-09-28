use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use cobblestone_codec::{Protocol84ChunkSnapshot, RawPacket, encode_protocol84_full_chunk_data};
use cobblestone_core::{
    Arena, CHUNK_NIBBLE_BYTES, ChunkCoord, ChunkEviction, ChunkPatch as NativeChunkPatch, Handle,
    RuntimeId, WorldStore,
};
use cobblestone_storage::{
    AsyncLoadConfig, AsyncLoadService, AsyncSaveConfig, AsyncSaveService, LoadCompletion,
    LoadRequestState, SaveCompletion, SaveSubmitError, WORLD_METADATA_FILENAME, WorldDirectory,
    WorldMetadata,
};
use ext_php_rs::binary::Binary;
use ext_php_rs::convert::IntoZval;
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;
use ext_php_rs::types::{ZendHashTable, Zval};

use crate::boundary::{php_boundary, php_error};
use crate::runtime::current_runtime_id;

const MAX_PROTOCOL84_CACHE_ENTRIES: usize = 4096;

static WORLD_ARENA: Mutex<Arena<NativeWorld>> = Mutex::new(Arena::new());

struct NativeWorld {
    owner: RuntimeId,
    state: Arc<NativeWorldState>,
}

struct NativeWorldState {
    store: Arc<WorldStore>,
    protocol84_cache: Mutex<HashMap<ChunkCoord, CachedProtocol84Chunk>>,
    persistence: Mutex<Option<NativeWorldPersistence>>,
}

struct NativeWorldPersistence {
    directory: WorldDirectory,
    saves: AsyncSaveService,
    loads: AsyncLoadService,
    save_in_flight: HashSet<ChunkCoord>,
    load_missing: HashSet<ChunkCoord>,
}

struct CachedProtocol84Chunk {
    terrain_revision: u64,
    light_revision: u64,
    packet: RawPacket,
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

fn resolve_world_state(handle_value: i64) -> PhpResult<Arc<NativeWorldState>> {
    let owner = current_runtime_id().map_err(php_error)?;
    let handle = handle(handle_value)?;
    let arena = world_arena();
    let world = arena
        .get(handle)
        .ok_or_else(|| php_error("native world handle is stale or unknown"))?;
    if world.owner != owner {
        return Err(php_error("native world belongs to another PHP runtime"));
    }
    Ok(Arc::clone(&world.state))
}

pub(crate) fn resolve_world(handle_value: i64) -> PhpResult<Arc<WorldStore>> {
    Ok(Arc::clone(&resolve_world_state(handle_value)?.store))
}

pub(crate) fn protocol84_chunk(handle_value: i64, position: ChunkCoord) -> PhpResult<RawPacket> {
    let state = resolve_world_state(handle_value)?;
    let snapshot = state
        .store
        .snapshot(position)
        .map_err(|error| php_error(error.to_string()))?;

    {
        let cache = match state.protocol84_cache.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(cached) = cache.get(&position)
            && cached.terrain_revision == snapshot.terrain_revision()
            && cached.light_revision == snapshot.light_revision()
        {
            return Ok(cached.packet.clone());
        }
    }

    let mut block_ids = Vec::with_capacity(snapshot.states().len());
    let mut block_data = vec![0_u8; CHUNK_NIBBLE_BYTES];
    for (index, &state_id) in snapshot.states().iter().enumerate() {
        block_ids.push((state_id >> 4) as u8);
        let data = (state_id & 0x0f) as u8;
        let byte = &mut block_data[index >> 1];
        if index & 1 == 0 {
            *byte = (*byte & 0xf0) | data;
        } else {
            *byte = (*byte & 0x0f) | (data << 4);
        }
    }

    let extra_data = snapshot
        .extra_data()
        .iter()
        .map(|(&key, &value)| (u32::from(key), value))
        .collect::<Vec<_>>();
    let packet = encode_protocol84_full_chunk_data(Protocol84ChunkSnapshot {
        chunk_x: position.x(),
        chunk_z: position.z(),
        block_ids: &block_ids,
        block_data: &block_data,
        sky_light: snapshot.sky_light(),
        block_light: snapshot.block_light(),
        biomes: snapshot.biomes(),
        height_map: snapshot.height_map(),
        extra_data: &extra_data,
    })
    .map_err(|error| php_error(error.to_string()))?;

    let current_terrain = state
        .store
        .terrain_revision(position)
        .map_err(|error| php_error(error.to_string()))?;
    let current_light = state
        .store
        .light_revision(position)
        .map_err(|error| php_error(error.to_string()))?;
    if current_terrain == snapshot.terrain_revision() && current_light == snapshot.light_revision()
    {
        let mut cache = match state.protocol84_cache.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if cache.len() >= MAX_PROTOCOL84_CACHE_ENTRIES && !cache.contains_key(&position) {
            cache.clear();
        }
        cache.insert(
            position,
            CachedProtocol84Chunk {
                terrain_revision: snapshot.terrain_revision(),
                light_revision: snapshot.light_revision(),
                packet: packet.clone(),
            },
        );
    }

    Ok(packet)
}

fn zval<T: IntoZval>(value: T) -> PhpResult<Zval> {
    value
        .into_zval(false)
        .map_err(|error| php_error(error.to_string()))
}

fn metadata_values(created: bool, metadata: &WorldMetadata) -> PhpResult<Vec<Zval>> {
    Ok(vec![
        zval(created)?,
        zval(Binary::new(metadata.world_uuid.to_vec()))?,
        zval(metadata.name.clone())?,
        zval(metadata.seed)?,
        zval(i64::from(metadata.generator_id))?,
        zval(i64::from(metadata.generator_settings_version))?,
        zval(Binary::new(metadata.generator_settings.clone()))?,
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

fn apply_save_completion(
    store: &WorldStore,
    persistence: &mut NativeWorldPersistence,
    completion: SaveCompletion,
) -> PhpResult<()> {
    let position = completion.position();
    persistence.save_in_flight.remove(&position);

    match completion {
        SaveCompletion::Saved(receipt) => store
            .mark_persisted(
                receipt.position,
                receipt.terrain_revision,
                receipt.light_revision,
                receipt.lifecycle_flags,
            )
            .map_err(|error| php_error(error.to_string())),
        SaveCompletion::Failed(failure) => Err(php_error(format!(
            "native world save failed for {}:{}: {}",
            failure.position.x(),
            failure.position.z(),
            failure.error
        ))),
    }
}

fn poll_load_completions(
    store: &WorldStore,
    persistence: &mut NativeWorldPersistence,
    budget: usize,
) -> PhpResult<usize> {
    let mut completed = 0;
    while completed < budget {
        let Some(completion) = persistence.loads.try_recv_completion() else {
            break;
        };

        match completion {
            LoadCompletion::Loaded(chunk) => {
                persistence.load_missing.remove(&chunk.position);
                store
                    .import_chunk_if_absent(chunk.position, chunk.import)
                    .map_err(|error| php_error(error.to_string()))?;
            }
            LoadCompletion::Missing(position) => {
                persistence.load_missing.insert(position);
            }
            LoadCompletion::Failed(failure) => {
                return Err(php_error(format!(
                    "native world load failed for {}:{}: {}",
                    failure.position.x(),
                    failure.position.z(),
                    failure.error
                )));
            }
        }
        completed += 1;
    }

    Ok(completed)
}

fn tick_persistence(
    store: &WorldStore,
    persistence: &mut NativeWorldPersistence,
    budget: usize,
) -> PhpResult<(usize, usize)> {
    let mut completed = 0;
    while completed < budget {
        let Some(completion) = persistence.saves.try_recv_completion() else {
            break;
        };
        apply_save_completion(store, persistence, completion)?;
        completed += 1;
    }

    let snapshots = store.dirty_snapshots_excluding(budget, &persistence.save_in_flight);
    let mut scheduled = 0;
    for snapshot in snapshots {
        let position = snapshot.position();
        match persistence.saves.try_save(snapshot) {
            Ok(()) => {
                persistence.save_in_flight.insert(position);
                scheduled += 1;
            }
            Err(SaveSubmitError::Backpressure(_)) => break,
            Err(SaveSubmitError::Closed(_)) => {
                return Err(php_error("native world save worker is closed"));
            }
        }
    }

    Ok((completed, scheduled))
}

fn flush_persistence(
    store: &WorldStore,
    persistence: &mut NativeWorldPersistence,
) -> PhpResult<()> {
    const FLUSH_BATCH: usize = 256;
    const COMPLETION_TIMEOUT: Duration = Duration::from_secs(30);

    loop {
        while let Some(completion) = persistence.saves.try_recv_completion() {
            apply_save_completion(store, persistence, completion)?;
        }

        let snapshots = store.dirty_snapshots_excluding(FLUSH_BATCH, &persistence.save_in_flight);
        let mut backpressured = false;
        for snapshot in snapshots {
            let position = snapshot.position();
            match persistence.saves.try_save(snapshot) {
                Ok(()) => {
                    persistence.save_in_flight.insert(position);
                }
                Err(SaveSubmitError::Backpressure(_)) => {
                    backpressured = true;
                    break;
                }
                Err(SaveSubmitError::Closed(_)) => {
                    return Err(php_error("native world save worker is closed"));
                }
            }
        }

        let remaining = store.dirty_snapshots_excluding(1, &persistence.save_in_flight);
        if remaining.is_empty() && persistence.save_in_flight.is_empty() {
            return Ok(());
        }

        if backpressured || !persistence.save_in_flight.is_empty() {
            let completion = persistence
                .saves
                .recv_completion_timeout(COMPLETION_TIMEOUT)
                .map_err(|_| php_error("timed out waiting for native world save completion"))?
                .ok_or_else(|| php_error("native world save workers stopped before flush"))?;
            apply_save_completion(store, persistence, completion)?;
        }
    }
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

fn parse_creation_metadata(values: &ZendHashTable) -> PhpResult<WorldMetadata> {
    if values.len() != 11 {
        return Err(php_error(
            "world storage creation metadata must contain exactly 11 values",
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

#[php_function]
pub fn cobblestone_world_create() -> PhpResult<i64> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let handle = world_arena()
            .insert(NativeWorld {
                owner,
                state: Arc::new(NativeWorldState {
                    store: Arc::new(WorldStore::new()),
                    protocol84_cache: Mutex::new(HashMap::new()),
                    persistence: Mutex::new(None),
                }),
            })
            .map_err(|_| php_error("native world handle capacity exhausted"))?;
        Ok(handle.into_raw() as i64)
    })
}

#[php_function]
pub fn cobblestone_world_storage_attach(
    handle_value: i64,
    root: String,
    creation: &ZendHashTable,
    save_workers: i64,
    load_workers: i64,
) -> PhpResult<Vec<Zval>> {
    php_boundary(|| {
        if root.is_empty() {
            return Err(php_error("native world storage root cannot be empty"));
        }
        let state = resolve_world_state(handle_value)?;
        let mut persistence = match state.persistence.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if persistence.is_some() {
            return Err(php_error("native world storage is already attached"));
        }

        let root = PathBuf::from(root);
        let metadata_path = root.join(WORLD_METADATA_FILENAME);
        let (directory, created) = if metadata_path.exists() {
            (
                WorldDirectory::open(&root).map_err(|error| php_error(error.to_string()))?,
                false,
            )
        } else {
            let metadata = parse_creation_metadata(creation)?;

            match WorldDirectory::create(&root, metadata) {
                Ok(directory) => (directory, true),
                Err(cobblestone_storage::StorageError::WorldMetadataAlreadyExists) => (
                    WorldDirectory::open(&root).map_err(|error| php_error(error.to_string()))?,
                    false,
                ),
                Err(error) => return Err(php_error(error.to_string())),
            }
        };

        let workers = usize::try_from(save_workers)
            .map_err(|_| php_error("save worker count must be positive"))?;
        let mut save_config = AsyncSaveConfig::new(directory.root(), directory.world_uuid());
        save_config.workers = workers;
        let saves =
            AsyncSaveService::start(save_config).map_err(|error| php_error(error.to_string()))?;

        let load_workers = usize::try_from(load_workers)
            .map_err(|_| php_error("load worker count must be positive"))?;
        let mut load_config = AsyncLoadConfig::new(directory.root(), directory.world_uuid());
        load_config.workers = load_workers;
        let loads =
            AsyncLoadService::start(load_config).map_err(|error| php_error(error.to_string()))?;
        let values = metadata_values(created, directory.metadata())?;

        *persistence = Some(NativeWorldPersistence {
            directory,
            saves,
            loads,
            save_in_flight: HashSet::new(),
            load_missing: HashSet::new(),
        });
        Ok(values)
    })
}

#[php_function]
pub fn cobblestone_world_storage_request_load(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
) -> PhpResult<i64> {
    php_boundary(|| {
        let position = position(chunk_x, chunk_z)?;
        let state = resolve_world_state(handle_value)?;
        if state.store.contains_chunk(position) {
            return Ok(0);
        }

        let mut persistence = match state.persistence.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let persistence = persistence
            .as_mut()
            .ok_or_else(|| php_error("native world storage is not attached"))?;

        if persistence.load_missing.contains(&position) {
            return Ok(3);
        }

        match persistence
            .loads
            .request(position)
            .map_err(|error| php_error(error.to_string()))?
        {
            LoadRequestState::Queued => Ok(1),
            LoadRequestState::Joined => Ok(2),
        }
    })
}

#[php_function]
pub fn cobblestone_world_storage_tick(handle_value: i64, budget: i64) -> PhpResult<Vec<Zval>> {
    php_boundary(|| {
        let budget = usize::try_from(budget)
            .ok()
            .filter(|&value| value > 0 && value <= 4096)
            .ok_or_else(|| php_error("world storage tick budget must be in range 1..4096"))?;
        let state = resolve_world_state(handle_value)?;
        let mut persistence = match state.persistence.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let Some(persistence) = persistence.as_mut() else {
            return Ok(vec![
                zval(0_i64)?,
                zval(0_i64)?,
                zval(0_i64)?,
                zval(0_i64)?,
                zval(0_i64)?,
                zval(0_i64)?,
                zval(0_i64)?,
            ]);
        };

        let load_completed = poll_load_completions(&state.store, persistence, budget)?;
        let (save_completed, save_scheduled) = tick_persistence(&state.store, persistence, budget)?;
        Ok(vec![
            zval(i64::try_from(save_completed).unwrap_or(i64::MAX))?,
            zval(i64::try_from(save_scheduled).unwrap_or(i64::MAX))?,
            zval(i64::try_from(persistence.save_in_flight.len()).unwrap_or(i64::MAX))?,
            zval(
                i64::try_from(persistence.directory.metadata().generation).map_err(|_| {
                    php_error("world metadata generation exceeds PHP integer range")
                })?,
            )?,
            zval(i64::try_from(load_completed).unwrap_or(i64::MAX))?,
            zval(i64::try_from(persistence.loads.in_flight()).unwrap_or(i64::MAX))?,
            zval(i64::try_from(persistence.load_missing.len()).unwrap_or(i64::MAX))?,
        ])
    })
}

#[php_function]
pub fn cobblestone_world_destroy(handle_value: i64) -> PhpResult<()> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let handle = handle(handle_value)?;
        let state = {
            let arena = world_arena();
            let world = arena
                .get(handle)
                .ok_or_else(|| php_error("native world handle is stale or unknown"))?;
            if world.owner != owner {
                return Err(php_error("native world belongs to another PHP runtime"));
            }
            let pins = world.state.store.total_pin_count();
            if pins != 0 {
                return Err(php_error(format!(
                    "native world cannot be destroyed while {pins} chunk pins are active"
                )));
            }
            Arc::clone(&world.state)
        };

        {
            let mut persistence = match state.persistence.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if let Some(persistence) = persistence.as_mut() {
                flush_persistence(&state.store, persistence)?;
            }
        }

        let mut arena = world_arena();
        let world = arena
            .get(handle)
            .ok_or_else(|| php_error("native world handle disappeared during shutdown"))?;
        if world.owner != owner {
            return Err(php_error("native world owner changed during shutdown"));
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
        let position = position(chunk_x, chunk_z)?;
        let state = resolve_world_state(handle_value)?;
        let inserted = state.store.ensure_chunk(position, byte(biome, "biome")?);
        if inserted {
            let mut persistence = match state.persistence.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if let Some(persistence) = persistence.as_mut() {
                persistence.load_missing.remove(&position);
            }
        }
        Ok(inserted)
    })
}

#[php_function]
pub fn cobblestone_world_lifecycle_flags(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
) -> PhpResult<i64> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        Ok(i64::from(
            store
                .lifecycle_flags(position(chunk_x, chunk_z)?)
                .map_err(|error| php_error(error.to_string()))?,
        ))
    })
}

#[php_function]
pub fn cobblestone_world_set_lifecycle_flags(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
    flags: i64,
) -> PhpResult<()> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        store
            .set_lifecycle_flags(
                position(chunk_x, chunk_z)?,
                byte(flags, "chunk lifecycle flags")?,
            )
            .map_err(|error| php_error(error.to_string()))
    })
}

#[php_function]
pub fn cobblestone_world_pin_chunk(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
) -> PhpResult<i64> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        Ok(i64::from(
            store
                .pin_chunk(position(chunk_x, chunk_z)?)
                .map_err(|error| php_error(error.to_string()))?,
        ))
    })
}

#[php_function]
pub fn cobblestone_world_unpin_chunk(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
) -> PhpResult<i64> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        Ok(i64::from(
            store
                .unpin_chunk(position(chunk_x, chunk_z)?)
                .map_err(|error| php_error(error.to_string()))?,
        ))
    })
}

#[php_function]
pub fn cobblestone_world_chunk_pin_count(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
) -> PhpResult<i64> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        Ok(i64::from(
            store
                .pin_count(position(chunk_x, chunk_z)?)
                .map_err(|error| php_error(error.to_string()))?,
        ))
    })
}

#[php_function]
pub fn cobblestone_world_chunk_dirty(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
) -> PhpResult<bool> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        store
            .is_dirty(position(chunk_x, chunk_z)?)
            .map_err(|error| php_error(error.to_string()))
    })
}

#[php_function]
pub fn cobblestone_world_mark_persisted(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
    terrain_revision: i64,
    light_revision: i64,
    lifecycle_flags: i64,
) -> PhpResult<()> {
    php_boundary(|| {
        let store = resolve_world(handle_value)?;
        store
            .mark_persisted(
                position(chunk_x, chunk_z)?,
                revision(terrain_revision, "persisted terrain revision")?,
                revision(light_revision, "persisted light revision")?,
                byte(lifecycle_flags, "persisted lifecycle flags")?,
            )
            .map_err(|error| php_error(error.to_string()))
    })
}

/// Attempts a safe residency eviction.
///
/// Return codes are stable at the PHP bridge: 0=missing, 1=pinned, 2=dirty, 3=evicted.
#[php_function]
pub fn cobblestone_world_try_evict_chunk(
    handle_value: i64,
    chunk_x: i64,
    chunk_z: i64,
) -> PhpResult<i64> {
    php_boundary(|| {
        let position = position(chunk_x, chunk_z)?;
        let state = resolve_world_state(handle_value)?;
        let result = state
            .store
            .try_evict_chunk(position)
            .map_err(|error| php_error(error.to_string()))?;

        if result == ChunkEviction::Evicted {
            let mut cache = match state.protocol84_cache.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            cache.remove(&position);
        }

        Ok(match result {
            ChunkEviction::Missing => 0,
            ChunkEviction::Pinned { .. } => 1,
            ChunkEviction::Dirty => 2,
            ChunkEviction::Evicted => 3,
        })
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
                .biome(
                    position(chunk_x, chunk_z)?,
                    local(x, "local x")?,
                    local(z, "local z")?,
                )
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

pub(crate) fn register(module: ModuleBuilder) -> ModuleBuilder {
    module
        .function(wrap_function!(cobblestone_world_create))
        .function(wrap_function!(cobblestone_world_storage_attach))
        .function(wrap_function!(cobblestone_world_storage_request_load))
        .function(wrap_function!(cobblestone_world_storage_tick))
        .function(wrap_function!(cobblestone_world_destroy))
        .function(wrap_function!(cobblestone_world_ensure_chunk))
        .function(wrap_function!(cobblestone_world_lifecycle_flags))
        .function(wrap_function!(cobblestone_world_set_lifecycle_flags))
        .function(wrap_function!(cobblestone_world_pin_chunk))
        .function(wrap_function!(cobblestone_world_unpin_chunk))
        .function(wrap_function!(cobblestone_world_chunk_pin_count))
        .function(wrap_function!(cobblestone_world_chunk_dirty))
        .function(wrap_function!(cobblestone_world_mark_persisted))
        .function(wrap_function!(cobblestone_world_try_evict_chunk))
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
    let worlds = world_arena().drain();

    for world in worlds {
        let mut persistence = match world.state.persistence.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(persistence) = persistence.as_mut() {
            let _ = flush_persistence(&world.state.store, persistence);
        }
    }
}
