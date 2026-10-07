use std::collections::{BTreeMap, HashMap};

use cobblestone_session::{SessionDelivery, SessionPacket, WorldViewSnapshot};
use cobblestone_target::ChunkShape;
use cobblestone_wire::{
    BatchPacket, BootstrapPacket, RawPacket, UPDATE_BLOCK_FLAG_ALL_PRIORITY,
    encode_bootstrap_packet, encode_update_block,
};
use cobblestone_world::{
    ChunkCoord, MAX_POINT_BLOCK_CHANGES, WorldChangeKind, WorldChangeLogSnapshot,
};
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::{php_boundary, php_error};
use crate::runtime::current_runtime_id;
use crate::world::{chunk_wire_packet, resolve_world};

use super::bridge::{QueueResult, codec_limits, try_queue, with_runtime};

const MAX_SYNC_BATCH_PACKETS: usize = 256;

#[derive(Debug, PartialEq, Eq)]
enum PendingChunkSync {
    Blocks(BTreeMap<u16, u16>),
    FullChunk,
}

fn merge_change(
    pending: &mut HashMap<ChunkCoord, PendingChunkSync>,
    position: ChunkCoord,
    kind: &WorldChangeKind,
) {
    match kind {
        WorldChangeKind::FullChunk => {
            pending.insert(position, PendingChunkSync::FullChunk);
        }
        WorldChangeKind::Blocks(changes) => {
            let entry = pending
                .entry(position)
                .or_insert_with(|| PendingChunkSync::Blocks(BTreeMap::new()));
            let PendingChunkSync::Blocks(blocks) = entry else {
                return;
            };
            for &(index, state) in changes {
                blocks.insert(index, state);
                if blocks.len() > MAX_POINT_BLOCK_CHANGES {
                    *entry = PendingChunkSync::FullChunk;
                    return;
                }
            }
        }
    }
}

fn pending_view_changes(
    view: &WorldViewSnapshot,
    log: &WorldChangeLogSnapshot,
) -> HashMap<ChunkCoord, PendingChunkSync> {
    let mut pending = HashMap::new();

    if log.cursor_is_stale(view.cursor()) {
        for &position in view.pinned_chunks() {
            pending.insert(position, PendingChunkSync::FullChunk);
        }
        return pending;
    }

    for change in log
        .changes()
        .iter()
        .filter(|change| change.sequence() > view.cursor())
    {
        if view.contains(change.position()) {
            merge_change(&mut pending, change.position(), change.kind());
        }
    }

    pending
}

fn update_block_packet(position: ChunkCoord, index: u16, state: u16) -> PhpResult<RawPacket> {
    let local_x = i32::from(index & 0x0f);
    let local_z = i32::from((index >> 4) & 0x0f);
    let y = u8::try_from((index >> 8) & 0x7f)
        .map_err(|_| php_error("world change y does not fit one byte"))?;
    let x = position
        .x()
        .checked_mul(ChunkShape::EDGE as i32)
        .and_then(|base| base.checked_add(local_x))
        .ok_or_else(|| php_error("UpdateBlock x coordinate overflow"))?;
    let z = position
        .z()
        .checked_mul(ChunkShape::EDGE as i32)
        .and_then(|base| base.checked_add(local_z))
        .ok_or_else(|| php_error("UpdateBlock z coordinate overflow"))?;

    encode_update_block(x, y, z, state, UPDATE_BLOCK_FLAG_ALL_PRIORITY)
        .map_err(|error| php_error(error.to_string()))
}

fn batch_packet(packets: Vec<RawPacket>) -> PhpResult<SessionPacket> {
    let raw = encode_bootstrap_packet(
        &BootstrapPacket::Batch(BatchPacket::new(packets)),
        codec_limits(),
    )
    .map_err(|error| php_error(error.to_string()))?;
    Ok(SessionPacket::new(raw.id(), raw.body().clone()))
}

/// Coalesces world changes and queues bounded reliable ordered Batches per spawned viewer.
#[php_function]
pub fn cobblestone_session_flush_world_changes(world_handle: i64) -> PhpResult<i64> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let store = resolve_world(world_handle)?;
        let views = with_runtime(owner, |host| Ok(host.world_view_snapshots(&store)))?;

        let latest = store.current_change_sequence();
        if views.is_empty() {
            store.prune_changes_through(latest);
            return Ok(0);
        }
        if views.iter().all(|view| view.cursor() == latest) {
            return Ok(0);
        }

        let log = store.change_log_snapshot();
        let mut cursor_updates = Vec::new();
        let mut queued_batches = 0_i64;

        for view in views {
            if view.cursor() == log.latest_sequence() {
                continue;
            }

            let pending = pending_view_changes(&view, &log);
            if pending.is_empty() {
                cursor_updates.push((view.session_id(), log.latest_sequence()));
                continue;
            }

            let mut chunks = pending.into_iter().collect::<Vec<_>>();
            chunks.sort_unstable_by_key(|(position, _)| (position.x(), position.z()));
            let mut packets = Vec::new();
            for (position, change) in chunks {
                match change {
                    PendingChunkSync::FullChunk => {
                        packets.push(chunk_wire_packet(world_handle, position)?);
                    }
                    PendingChunkSync::Blocks(blocks) => {
                        for (index, state) in blocks {
                            packets.push(update_block_packet(position, index, state)?);
                        }
                    }
                }
            }

            let mut completed = true;
            for packet_batch in packets.chunks(MAX_SYNC_BATCH_PACKETS) {
                match try_queue(
                    owner,
                    view.session_id(),
                    batch_packet(packet_batch.to_vec())?,
                    SessionDelivery::ReliableOrdered,
                )? {
                    QueueResult::Sent => queued_batches = queued_batches.saturating_add(1),
                    QueueResult::Backpressured | QueueResult::Gone => {
                        completed = false;
                        break;
                    }
                }
            }
            if completed {
                cursor_updates.push((view.session_id(), log.latest_sequence()));
            }
        }

        for (session_id, cursor) in cursor_updates {
            let _ = with_runtime(owner, |host| {
                Ok(host.update_view_cursor(session_id, &store, cursor))
            })?;
        }

        let prune_through = with_runtime(owner, |host| Ok(host.world_view_snapshots(&store)))?
            .iter()
            .map(WorldViewSnapshot::cursor)
            .min()
            .unwrap_or(log.latest_sequence());
        store.prune_changes_through(prune_through);

        Ok(queued_batches)
    })
}

pub(crate) fn register(module: ModuleBuilder) -> ModuleBuilder {
    module.function(wrap_function!(cobblestone_session_flush_world_changes))
}
