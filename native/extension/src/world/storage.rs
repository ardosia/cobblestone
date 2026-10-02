use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::time::Duration;

use cobblestone_storage::{
    AsyncLoadConfig, AsyncLoadService, AsyncSaveConfig, AsyncSaveService, CompactionCompletion,
    CompactionSubmitError, LoadCompletion, LoadRequestState, RegionCoord, RegionStats,
    SaveCompletion, SaveSubmitError, WORLD_METADATA_FILENAME, WorldDirectory, WorldMetadata,
};
use cobblestone_world::{ChunkCoord, WorldStore};
use ext_php_rs::binary::Binary;
use ext_php_rs::convert::IntoZval;
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;
use ext_php_rs::types::{ZendHashTable, Zval};

use crate::boundary::{php_boundary, php_error};

use super::{position, resolve_world_state};

const MAX_COMPACTION_CANDIDATES: usize = 4096;

#[derive(Clone, Copy)]
struct NativeCompactionPolicy {
    min_dead_bytes: u64,
    min_dead_percent: u8,
}

impl NativeCompactionPolicy {
    fn matches(self, stats: RegionStats) -> bool {
        if stats.dead_bytes == 0 || stats.record_bytes == 0 {
            return false;
        }

        let bytes_match = self.min_dead_bytes == 0 || stats.dead_bytes >= self.min_dead_bytes;
        let percent_match = self.min_dead_percent == 0
            || u128::from(stats.dead_bytes) * 100
                >= u128::from(stats.record_bytes) * u128::from(self.min_dead_percent);
        bytes_match && percent_match
    }
}

pub(super) struct NativeWorldPersistence {
    directory: WorldDirectory,
    saves: AsyncSaveService,
    loads: AsyncLoadService,
    save_in_flight: HashSet<ChunkCoord>,
    load_missing: HashSet<ChunkCoord>,
    save_bytes_appended: u64,
    region_stats: HashMap<RegionCoord, RegionStats>,
    compaction_policy: Option<NativeCompactionPolicy>,
    compaction_queue: VecDeque<RegionCoord>,
    compaction_queued: HashSet<RegionCoord>,
    compaction_in_flight: HashSet<RegionCoord>,
    compaction_blocked: HashSet<RegionCoord>,
    compactions_completed: u64,
    compactions_failed: u64,
    compaction_bytes_reclaimed: u64,
    compaction_last_error: String,
}

impl NativeWorldPersistence {
    pub(super) fn clear_missing(&mut self, position: ChunkCoord) {
        self.load_missing.remove(&position);
    }
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

fn maybe_queue_compaction(
    persistence: &mut NativeWorldPersistence,
    region: RegionCoord,
    stats: RegionStats,
) {
    let Some(policy) = persistence.compaction_policy else {
        return;
    };
    if !policy.matches(stats)
        || persistence.compaction_blocked.contains(&region)
        || persistence.compaction_in_flight.contains(&region)
        || persistence.compaction_queued.contains(&region)
        || persistence.compaction_queue.len() >= MAX_COMPACTION_CANDIDATES
    {
        return;
    }

    persistence.compaction_queued.insert(region);
    persistence.compaction_queue.push_back(region);
}

fn apply_save_completion(
    store: &WorldStore,
    persistence: &mut NativeWorldPersistence,
    completion: SaveCompletion,
) -> PhpResult<()> {
    let position = completion.position();
    persistence.save_in_flight.remove(&position);

    match completion {
        SaveCompletion::Saved(receipt) => {
            store
                .mark_persisted(
                    receipt.position,
                    receipt.terrain_revision,
                    receipt.light_revision,
                    receipt.lifecycle_flags,
                )
                .map_err(|error| php_error(error.to_string()))?;
            persistence.save_bytes_appended = persistence
                .save_bytes_appended
                .checked_add(receipt.bytes_appended)
                .ok_or_else(|| php_error("world storage append byte count overflow"))?;
            persistence
                .region_stats
                .insert(receipt.region, receipt.region_stats);
            maybe_queue_compaction(persistence, receipt.region, receipt.region_stats);
            Ok(())
        }
        SaveCompletion::Failed(failure) => Err(php_error(format!(
            "native world save failed for {}:{}: {}",
            failure.position.x(),
            failure.position.z(),
            failure.error
        ))),
    }
}

fn apply_compaction_completion(
    persistence: &mut NativeWorldPersistence,
    completion: CompactionCompletion,
) -> PhpResult<bool> {
    match completion {
        CompactionCompletion::Compacted(receipt) => {
            persistence.compaction_in_flight.remove(&receipt.region);
            persistence
                .region_stats
                .insert(receipt.region, receipt.after);
            persistence.compactions_completed = persistence
                .compactions_completed
                .checked_add(1)
                .ok_or_else(|| php_error("world storage compaction completion count overflow"))?;
            persistence.compaction_bytes_reclaimed = persistence
                .compaction_bytes_reclaimed
                .checked_add(receipt.bytes_reclaimed)
                .ok_or_else(|| php_error("world storage reclaimed byte count overflow"))?;
            Ok(false)
        }
        CompactionCompletion::Failed(failure) => {
            persistence.compaction_in_flight.remove(&failure.region);
            persistence.compaction_blocked.insert(failure.region);
            persistence.compactions_failed = persistence
                .compactions_failed
                .checked_add(1)
                .ok_or_else(|| php_error("world storage compaction failure count overflow"))?;
            persistence.compaction_last_error = format!(
                "region {}:{}: {}",
                failure.region.x, failure.region.z, failure.error
            );
            Ok(true)
        }
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

struct PersistenceTickResult {
    save_completed: usize,
    save_scheduled: usize,
    compaction_completed: usize,
    compaction_failed: usize,
    compaction_scheduled: usize,
}

fn tick_persistence(
    store: &WorldStore,
    persistence: &mut NativeWorldPersistence,
    budget: usize,
) -> PhpResult<PersistenceTickResult> {
    let mut compaction_completed = 0;
    let mut compaction_failed = 0;
    while compaction_completed + compaction_failed < budget {
        let Some(completion) = persistence.saves.try_recv_compaction() else {
            break;
        };
        if apply_compaction_completion(persistence, completion)? {
            compaction_failed += 1;
        } else {
            compaction_completed += 1;
        }
    }

    let mut save_completed = 0;
    while save_completed < budget {
        let Some(completion) = persistence.saves.try_recv_completion() else {
            break;
        };
        apply_save_completion(store, persistence, completion)?;
        save_completed += 1;
    }

    let snapshots = store.dirty_snapshots_excluding(budget, &persistence.save_in_flight);
    let mut save_scheduled = 0;
    for snapshot in snapshots {
        let position = snapshot.position();
        match persistence.saves.try_save(snapshot) {
            Ok(()) => {
                persistence.save_in_flight.insert(position);
                save_scheduled += 1;
            }
            Err(SaveSubmitError::Backpressure(_)) => break,
            Err(SaveSubmitError::Closed(_)) => {
                return Err(php_error("native world save worker is closed"));
            }
        }
    }

    let mut compaction_scheduled = 0;
    while compaction_scheduled < budget {
        let Some(region) = persistence.compaction_queue.pop_front() else {
            break;
        };
        persistence.compaction_queued.remove(&region);

        if persistence.compaction_blocked.contains(&region)
            || persistence.compaction_in_flight.contains(&region)
        {
            continue;
        }
        let Some(stats) = persistence.region_stats.get(&region).copied() else {
            continue;
        };
        let Some(policy) = persistence.compaction_policy else {
            continue;
        };
        if !policy.matches(stats) {
            continue;
        }

        match persistence.saves.try_compact(region) {
            Ok(()) => {
                persistence.compaction_in_flight.insert(region);
                compaction_scheduled += 1;
            }
            Err(CompactionSubmitError::Backpressure(region)) => {
                persistence.compaction_queued.insert(region);
                persistence.compaction_queue.push_front(region);
                break;
            }
            Err(CompactionSubmitError::Closed(_)) => {
                return Err(php_error("native world storage worker is closed"));
            }
        }
    }

    Ok(PersistenceTickResult {
        save_completed,
        save_scheduled,
        compaction_completed,
        compaction_failed,
        compaction_scheduled,
    })
}

pub(super) fn flush_persistence(
    store: &WorldStore,
    persistence: &mut NativeWorldPersistence,
) -> PhpResult<()> {
    const FLUSH_BATCH: usize = 256;
    const COMPLETION_TIMEOUT: Duration = Duration::from_secs(30);

    loop {
        while let Some(completion) = persistence.saves.try_recv_compaction() {
            let _ = apply_compaction_completion(persistence, completion)?;
        }
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

fn decode_chunk_positions(input: &[u8]) -> PhpResult<Vec<ChunkCoord>> {
    const HEADER_BYTES: usize = 4;
    const ENTRY_BYTES: usize = 8;
    const MAX_POSITIONS: usize = 4096;

    if input.len() < HEADER_BYTES {
        return Err(php_error("chunk position batch is truncated"));
    }
    let count = u32::from_le_bytes(
        input[0..4]
            .try_into()
            .expect("fixed four-byte chunk batch header"),
    ) as usize;
    if count > MAX_POSITIONS {
        return Err(php_error("chunk position batch exceeds 4096 entries"));
    }
    let expected = HEADER_BYTES
        .checked_add(
            count
                .checked_mul(ENTRY_BYTES)
                .ok_or_else(|| php_error("chunk position batch length overflow"))?,
        )
        .ok_or_else(|| php_error("chunk position batch length overflow"))?;
    if input.len() != expected {
        return Err(php_error("chunk position batch length mismatch"));
    }

    let mut positions = Vec::with_capacity(count);
    let mut offset = HEADER_BYTES;
    for _ in 0..count {
        let x = i32::from_le_bytes(
            input[offset..offset + 4]
                .try_into()
                .expect("fixed chunk x slice"),
        );
        let z = i32::from_le_bytes(
            input[offset + 4..offset + 8]
                .try_into()
                .expect("fixed chunk z slice"),
        );
        positions.push(ChunkCoord::new(x, z));
        offset += ENTRY_BYTES;
    }

    Ok(positions)
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
pub fn cobblestone_world_storage_attach(
    handle_value: i64,
    root: String,
    creation: &ZendHashTable,
    save_workers: i64,
    load_workers: i64,
    compaction_min_dead_bytes: i64,
    compaction_min_dead_percent: i64,
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

        let compaction_min_dead_bytes = u64::try_from(compaction_min_dead_bytes)
            .map_err(|_| php_error("compaction minimum dead bytes must be nonnegative"))?;
        let compaction_min_dead_percent = u8::try_from(compaction_min_dead_percent)
            .ok()
            .filter(|&value| value <= 100)
            .ok_or_else(|| php_error("compaction minimum dead percent must be in range 0..100"))?;
        let compaction_policy =
            if compaction_min_dead_bytes == 0 && compaction_min_dead_percent == 0 {
                None
            } else {
                Some(NativeCompactionPolicy {
                    min_dead_bytes: compaction_min_dead_bytes,
                    min_dead_percent: compaction_min_dead_percent,
                })
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
            save_bytes_appended: 0,
            region_stats: HashMap::new(),
            compaction_policy,
            compaction_queue: VecDeque::new(),
            compaction_queued: HashSet::new(),
            compaction_in_flight: HashSet::new(),
            compaction_blocked: HashSet::new(),
            compactions_completed: 0,
            compactions_failed: 0,
            compaction_bytes_reclaimed: 0,
            compaction_last_error: String::new(),
        });
        Ok(values)
    })
}

#[php_function]
pub fn cobblestone_world_storage_prepare_loads(
    handle_value: i64,
    positions: Binary<u8>,
) -> PhpResult<Binary<u8>> {
    php_boundary(|| {
        let positions = decode_chunk_positions(positions.as_slice())?;
        let state = resolve_world_state(handle_value)?;
        let mut persistence = match state.persistence.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let persistence = persistence
            .as_mut()
            .ok_or_else(|| php_error("native world storage is not attached"))?;

        poll_load_completions(&state.store, persistence, positions.len().max(1))?;

        let mut statuses = Vec::with_capacity(positions.len());
        for position in positions {
            if state.store.contains_chunk(position) {
                statuses.push(0);
                continue;
            }
            if persistence.load_missing.contains(&position) {
                statuses.push(3);
                continue;
            }

            match persistence
                .loads
                .request(position)
                .map_err(|error| php_error(error.to_string()))?
            {
                LoadRequestState::Queued => statuses.push(1),
                LoadRequestState::Joined => statuses.push(2),
            }
        }

        Ok(Binary::new(statuses))
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
                zval(0_i64)?,
                zval(0_i64)?,
                zval(0_i64)?,
                zval(0_i64)?,
                zval(0_i64)?,
            ]);
        };

        let load_completed = poll_load_completions(&state.store, persistence, budget)?;
        let tick = tick_persistence(&state.store, persistence, budget)?;
        Ok(vec![
            zval(i64::try_from(tick.save_completed).unwrap_or(i64::MAX))?,
            zval(i64::try_from(tick.save_scheduled).unwrap_or(i64::MAX))?,
            zval(i64::try_from(persistence.save_in_flight.len()).unwrap_or(i64::MAX))?,
            zval(
                i64::try_from(persistence.directory.metadata().generation).map_err(|_| {
                    php_error("world metadata generation exceeds PHP integer range")
                })?,
            )?,
            zval(i64::try_from(load_completed).unwrap_or(i64::MAX))?,
            zval(i64::try_from(persistence.loads.in_flight()).unwrap_or(i64::MAX))?,
            zval(i64::try_from(persistence.load_missing.len()).unwrap_or(i64::MAX))?,
            zval(i64::try_from(tick.compaction_completed).unwrap_or(i64::MAX))?,
            zval(i64::try_from(tick.compaction_failed).unwrap_or(i64::MAX))?,
            zval(i64::try_from(tick.compaction_scheduled).unwrap_or(i64::MAX))?,
            zval(i64::try_from(persistence.compaction_in_flight.len()).unwrap_or(i64::MAX))?,
            zval(i64::try_from(persistence.compaction_queue.len()).unwrap_or(i64::MAX))?,
        ])
    })
}

#[php_function]
pub fn cobblestone_world_storage_stats(handle_value: i64) -> PhpResult<Vec<Zval>> {
    php_boundary(|| {
        let state = resolve_world_state(handle_value)?;
        let persistence = match state.persistence.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let Some(persistence) = persistence.as_ref() else {
            return Ok(vec![
                zval(0_i64)?,
                zval(0_i64)?,
                zval(0_i64)?,
                zval(0_i64)?,
                zval(0_i64)?,
                zval(0_i64)?,
                zval(0_i64)?,
                zval(0_i64)?,
                zval(0_i64)?,
                zval(String::new())?,
            ]);
        };

        let record_bytes = persistence
            .region_stats
            .values()
            .try_fold(0_u64, |total, stats| {
                total
                    .checked_add(stats.record_bytes)
                    .ok_or_else(|| php_error("world storage record byte count overflow"))
            })?;
        let live_bytes = persistence
            .region_stats
            .values()
            .try_fold(0_u64, |total, stats| {
                total
                    .checked_add(stats.live_bytes)
                    .ok_or_else(|| php_error("world storage live byte count overflow"))
            })?;
        let dead_bytes = persistence
            .region_stats
            .values()
            .try_fold(0_u64, |total, stats| {
                total
                    .checked_add(stats.dead_bytes)
                    .ok_or_else(|| php_error("world storage dead byte count overflow"))
            })?;

        let as_php_int = |value: u64, field: &'static str| {
            i64::try_from(value)
                .map_err(|_| php_error(format!("world storage {field} exceeds PHP integer range")))
        };

        Ok(vec![
            zval(as_php_int(
                persistence.save_bytes_appended,
                "append byte count",
            )?)?,
            zval(i64::try_from(persistence.region_stats.len()).unwrap_or(i64::MAX))?,
            zval(as_php_int(record_bytes, "record byte count")?)?,
            zval(as_php_int(live_bytes, "live byte count")?)?,
            zval(as_php_int(dead_bytes, "dead byte count")?)?,
            zval(as_php_int(
                persistence.compactions_completed,
                "compaction completion count",
            )?)?,
            zval(as_php_int(
                persistence.compactions_failed,
                "compaction failure count",
            )?)?,
            zval(as_php_int(
                persistence.compaction_bytes_reclaimed,
                "compaction reclaimed byte count",
            )?)?,
            zval(i64::try_from(persistence.compaction_blocked.len()).unwrap_or(i64::MAX))?,
            zval(persistence.compaction_last_error.clone())?,
        ])
    })
}

#[php_function]
pub fn cobblestone_world_storage_flush(handle_value: i64) -> PhpResult<()> {
    php_boundary(|| {
        let state = resolve_world_state(handle_value)?;
        let mut persistence = match state.persistence.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(persistence) = persistence.as_mut() {
            flush_persistence(&state.store, persistence)?;
        }
        Ok(())
    })
}

pub(super) fn register(module: ModuleBuilder) -> ModuleBuilder {
    module
        .function(wrap_function!(cobblestone_world_storage_attach))
        .function(wrap_function!(cobblestone_world_storage_prepare_loads))
        .function(wrap_function!(cobblestone_world_storage_request_load))
        .function(wrap_function!(cobblestone_world_storage_tick))
        .function(wrap_function!(cobblestone_world_storage_stats))
        .function(wrap_function!(cobblestone_world_storage_flush))
}
