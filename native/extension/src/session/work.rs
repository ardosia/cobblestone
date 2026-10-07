use std::time::Instant;

use cobblestone_session::{ChunkWorkCompletion, ChunkWorkKind, SessionWorldBootstrap};
use cobblestone_wire::DimensionId;
use cobblestone_world::ChunkCoord;
use ext_php_rs::convert::IntoZval;
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;
use ext_php_rs::types::{ZendHashTable, Zval};

use crate::boundary::{php_boundary, php_error};
use crate::runtime::current_runtime_id;
use crate::world::{chunk_wire_packet, resolve_world};

use super::bridge::{owner_session_id, with_runtime, zval};

fn bootstrap_value<'a>(
    values: &'a ZendHashTable,
    index: i64,
    field: &'static str,
) -> PhpResult<&'a Zval> {
    values
        .get_index(index)
        .ok_or_else(|| php_error(format!("session Login bootstrap is missing {field}")))
}

fn bootstrap_long(values: &ZendHashTable, index: i64, field: &'static str) -> PhpResult<i64> {
    bootstrap_value(values, index, field)?
        .long()
        .ok_or_else(|| {
            php_error(format!(
                "session Login bootstrap {field} must be an integer"
            ))
        })
}

fn parse_bootstrap(values: &ZendHashTable) -> PhpResult<SessionWorldBootstrap> {
    if values.len() != 9 {
        return Err(php_error(
            "session Login bootstrap must contain exactly 9 semantic values",
        ));
    }

    let dimension = u8::try_from(bootstrap_long(values, 2, "dimension id")?)
        .ok()
        .and_then(|value| DimensionId::try_from(value).ok())
        .ok_or_else(|| php_error("fixed-target dimension id must be 0 or 1"))?;
    let time_started = bootstrap_value(values, 7, "time-running flag")?
        .bool()
        .ok_or_else(|| php_error("session Login bootstrap time-running flag must be boolean"))?;
    let level_id = bootstrap_value(values, 8, "level id")?
        .string()
        .ok_or_else(|| php_error("session Login bootstrap level id must be a string"))?;

    SessionWorldBootstrap::new(
        i32::try_from(bootstrap_long(values, 0, "world seed")?)
            .map_err(|_| php_error("world seed must fit signed 32 bits"))?,
        dimension,
        i32::try_from(bootstrap_long(values, 1, "generator id")?)
            .map_err(|_| php_error("generator id must fit signed 32 bits"))?,
        [
            i32::try_from(bootstrap_long(values, 3, "spawn x")?)
                .map_err(|_| php_error("spawn x must fit signed 32 bits"))?,
            i32::try_from(bootstrap_long(values, 4, "spawn y")?)
                .map_err(|_| php_error("spawn y must fit signed 32 bits"))?,
            i32::try_from(bootstrap_long(values, 5, "spawn z")?)
                .map_err(|_| php_error("spawn z must fit signed 32 bits"))?,
        ],
        i32::try_from(bootstrap_long(values, 6, "world time")?)
            .map_err(|_| php_error("world time must fit signed 32 bits"))?,
        time_started,
        level_id,
    )
    .map_err(php_error)
}

#[php_function]
pub fn cobblestone_session_accept_login(
    session_id: i64,
    bootstrap: &ZendHashTable,
) -> PhpResult<()> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let session_id = owner_session_id(session_id)?;
        let bootstrap = parse_bootstrap(bootstrap)?;
        with_runtime(owner, |host| host.accept_login(session_id, bootstrap))
    })
}

/// Returns [sessionId, kind, [x0, z0, x1, z1, ...]] for the next pending chunk-work item.
#[php_function]
pub fn cobblestone_session_next_chunk_work(after_session_id: i64) -> PhpResult<Option<Vec<Zval>>> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let after = if after_session_id <= 0 {
            None
        } else {
            Some(owner_session_id(after_session_id)?)
        };
        let work = with_runtime(owner, |host| Ok(host.next_chunk_work(after)))?;
        let Some(work) = work else {
            return Ok(None);
        };
        let kind = match work.kind() {
            ChunkWorkKind::Initial => 0_i64,
            ChunkWorkKind::View => 1_i64,
        };
        let mut positions = Vec::with_capacity(work.positions().len() * 2);
        for position in work.positions() {
            positions.push(i64::from(position.x()));
            positions.push(i64::from(position.z()));
        }
        Ok(Some(vec![
            zval(work.session_id().get())?,
            zval(kind)?,
            positions
                .into_zval(false)
                .map_err(|error| php_error(error.to_string()))?,
        ]))
    })
}

#[php_function]
pub fn cobblestone_session_mark_chunk_prepared(
    session_id: i64,
    world_handle: i64,
    chunk_x: i64,
    chunk_z: i64,
) -> PhpResult<()> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let session_id = owner_session_id(session_id)?;
        let position = ChunkCoord::new(
            i32::try_from(chunk_x).map_err(|_| php_error("chunk x must fit signed 32 bits"))?,
            i32::try_from(chunk_z).map_err(|_| php_error("chunk z must fit signed 32 bits"))?,
        );
        let store = resolve_world(world_handle)?;
        with_runtime(owner, |host| {
            host.mark_chunk_prepared(session_id, store, position)
        })
    })
}

/// Returns [status, requestedRadius, effectiveRadius, chunksSent, encodedBytes, encodeNanos].
/// status: 0=backpressured, 1=complete view, 2=spawned, 3=gone.
#[php_function]
pub fn cobblestone_session_complete_chunk_work(
    session_id: i64,
    world_handle: i64,
) -> PhpResult<Vec<Zval>> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let session_id = owner_session_id(session_id)?;
        let work = with_runtime(owner, |host| host.chunk_work(session_id))?
            .ok_or_else(|| php_error("session has no pending chunk work"))?;
        let store = resolve_world(world_handle)?;
        let started = Instant::now();
        let packets = work
            .send_positions()
            .iter()
            .copied()
            .map(|position| chunk_wire_packet(world_handle, position))
            .collect::<PhpResult<Vec<_>>>()?;
        let completion = with_runtime(owner, |host| {
            host.try_complete_chunk_work(session_id, &store, packets)
        })?;
        let elapsed = i64::try_from(started.elapsed().as_nanos()).unwrap_or(i64::MAX);

        match completion {
            ChunkWorkCompletion::Backpressured => result_values(0, 0, 0, 0, 0, elapsed),
            ChunkWorkCompletion::Complete => result_values(1, 0, 0, 0, 0, elapsed),
            ChunkWorkCompletion::Spawned(result) => result_values(
                2,
                i64::from(result.requested_radius()),
                i64::from(result.effective_radius()),
                i64::try_from(result.chunks_sent()).unwrap_or(i64::MAX),
                i64::try_from(result.encoded_bytes()).unwrap_or(i64::MAX),
                elapsed,
            ),
            ChunkWorkCompletion::Gone => result_values(3, 0, 0, 0, 0, elapsed),
        }
    })
}

fn result_values(
    status: i64,
    requested_radius: i64,
    effective_radius: i64,
    chunks_sent: i64,
    encoded_bytes: i64,
    encode_nanos: i64,
) -> PhpResult<Vec<Zval>> {
    Ok(vec![
        zval(status)?,
        zval(requested_radius)?,
        zval(effective_radius)?,
        zval(chunks_sent)?,
        zval(encoded_bytes)?,
        zval(encode_nanos)?,
    ])
}

pub(crate) fn register(module: ModuleBuilder) -> ModuleBuilder {
    module
        .function(wrap_function!(cobblestone_session_accept_login))
        .function(wrap_function!(cobblestone_session_next_chunk_work))
        .function(wrap_function!(cobblestone_session_mark_chunk_prepared))
        .function(wrap_function!(cobblestone_session_complete_chunk_work))
}
