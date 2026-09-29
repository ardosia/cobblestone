use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard};

use super::{ChunkCoord, WorldStore};

pub const WORLD_CHANGE_LOG_CAPACITY: usize = 8192;
pub const MAX_POINT_BLOCK_CHANGES: usize = 256;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum WorldChangeKind {
    Blocks(Vec<(u16, u16)>),
    FullChunk,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WorldChange {
    sequence: u64,
    position: ChunkCoord,
    kind: WorldChangeKind,
}

impl WorldChange {
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn position(&self) -> ChunkCoord {
        self.position
    }

    pub const fn kind(&self) -> &WorldChangeKind {
        &self.kind
    }
}

#[derive(Debug, Clone)]
pub struct WorldChangeLogSnapshot {
    oldest_sequence: u64,
    latest_sequence: u64,
    changes: Vec<WorldChange>,
}

impl WorldChangeLogSnapshot {
    pub const fn oldest_sequence(&self) -> u64 {
        self.oldest_sequence
    }

    pub const fn latest_sequence(&self) -> u64 {
        self.latest_sequence
    }

    pub fn changes(&self) -> &[WorldChange] {
        &self.changes
    }

    pub fn cursor_is_stale(&self, cursor: u64) -> bool {
        cursor.saturating_add(1) < self.oldest_sequence
    }
}

#[derive(Debug)]
pub(super) struct WorldChangeLog {
    next_sequence: u64,
    changes: VecDeque<WorldChange>,
}

impl Default for WorldChangeLog {
    fn default() -> Self {
        Self {
            next_sequence: 1,
            changes: VecDeque::with_capacity(WORLD_CHANGE_LOG_CAPACITY),
        }
    }
}

fn lock_changes(lock: &Mutex<WorldChangeLog>) -> MutexGuard<'_, WorldChangeLog> {
    match lock.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

impl WorldStore {
    pub fn current_change_sequence(&self) -> u64 {
        lock_changes(&self.changes).next_sequence.saturating_sub(1)
    }

    pub fn change_log_snapshot(&self) -> WorldChangeLogSnapshot {
        let changes = lock_changes(&self.changes);
        let latest_sequence = changes.next_sequence.saturating_sub(1);
        let oldest_sequence = changes
            .changes
            .front()
            .map(WorldChange::sequence)
            .unwrap_or_else(|| latest_sequence.saturating_add(1));

        WorldChangeLogSnapshot {
            oldest_sequence,
            latest_sequence,
            changes: changes.changes.iter().cloned().collect(),
        }
    }

    pub fn prune_changes_through(&self, sequence: u64) {
        let mut changes = lock_changes(&self.changes);
        while changes
            .changes
            .front()
            .is_some_and(|change| change.sequence <= sequence)
        {
            changes.changes.pop_front();
        }
    }

    pub(super) fn record_change(&self, position: ChunkCoord, kind: WorldChangeKind) {
        let mut changes = lock_changes(&self.changes);
        let sequence = changes.next_sequence;
        changes.next_sequence = changes
            .next_sequence
            .checked_add(1)
            .expect("world change sequence exhausted");

        if changes.changes.len() == WORLD_CHANGE_LOG_CAPACITY {
            changes.changes.pop_front();
        }
        changes.changes.push_back(WorldChange {
            sequence,
            position,
            kind,
        });
    }
}
