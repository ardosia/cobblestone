use std::time::Duration;

use cobblestone_storage::{
    CompactionCompletion, CompactionSubmitError, RegionCoord, RegionStats, SaveCompletion,
    SaveSubmitError,
};
use cobblestone_world::WorldStore;
use ext_php_rs::exception::PhpResult;

use crate::boundary::php_error;

use super::{MAX_COMPACTION_CANDIDATES, NativeWorldPersistence};

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

pub(super) struct PersistenceTickResult {
    pub(super) save_completed: usize,
    pub(super) save_scheduled: usize,
    pub(super) compaction_completed: usize,
    pub(super) compaction_failed: usize,
    pub(super) compaction_scheduled: usize,
}

pub(super) fn tick_persistence(
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

pub(in crate::world) fn flush_persistence(
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
