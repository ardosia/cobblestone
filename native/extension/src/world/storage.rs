use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;

use cobblestone_storage::{
    AsyncLoadConfig, AsyncLoadService, AsyncSaveConfig, AsyncSaveService, LoadRequestState,
    RegionCoord, RegionStats, WORLD_METADATA_FILENAME, WorldDirectory,
};
use cobblestone_world::ChunkCoord;
use ext_php_rs::binary::Binary;
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;
use ext_php_rs::types::{ZendHashTable, Zval};

use crate::boundary::{php_boundary, php_error};

use super::{position, resolve_world_state};

mod loads;
mod metadata;
mod tick;

use loads::{decode_chunk_positions, poll_load_completions};
use metadata::{metadata_values, parse_creation_metadata, zval};
pub(super) use tick::flush_persistence;
use tick::tick_persistence;

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
