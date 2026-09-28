use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

use cobblestone_codec::{
    AdventureFlags, AdventureSettingsPacket, BatchPacket, BootstrapPacket, CHUNK_BLOCK_COUNT,
    CHUNK_COLUMN_COUNT, CHUNK_NIBBLE_BYTES, PlayStatusPacket, Protocol84ChunkSnapshot, RawPacket,
    SetDifficultyPacket, SetSpawnPositionPacket, SetTimePacket, StartGamePacket,
    UPDATE_BLOCK_FLAG_ALL_PRIORITY, decode_bootstrap_packet, encode_bootstrap_packet,
    encode_protocol84_full_chunk_data, encode_protocol84_update_block, packet_id,
};
use cobblestone_core::{
    ChunkCoord, MAX_POINT_BLOCK_CHANGES, NativeBuffer, RuntimeId, WorldChangeKind, WorldStore,
};
use cobblestone_session::{SessionDelivery, SessionId, SessionPacket};
use ext_php_rs::binary::Binary;
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::{php_boundary, php_error};
use crate::runtime::current_runtime_id;
use crate::session::bridge::{
    QueueResult, codec_limits, owner_session_id, try_queue, with_runtime,
};
use crate::world::{protocol84_chunk, resolve_world};

const MAX_INITIAL_CHUNK_RADIUS: i32 = 3;
const MAX_INITIAL_CHUNKS: usize = 49;
const MAX_PROJECTION_BYTES: usize = 4 * 1024 * 1024;
const MAX_SYNC_BATCH_PACKETS: usize = 256;
const CHUNK_RADIUS_UPDATED_ID: u8 = 0x3e;

#[derive(Debug, Clone)]
struct WorldView {
    world_handle: i64,
    store: Arc<WorldStore>,
    center: ChunkCoord,
    radius: i32,
    cursor: u64,
    pinned_chunks: Vec<ChunkCoord>,
}

fn view_contains(center: ChunkCoord, radius: i32, position: ChunkCoord) -> bool {
    position.x() >= center.x().saturating_sub(radius)
        && position.x() <= center.x().saturating_add(radius)
        && position.z() >= center.z().saturating_sub(radius)
        && position.z() <= center.z().saturating_add(radius)
}

fn view_positions(center: ChunkCoord, radius: i32) -> Vec<ChunkCoord> {
    debug_assert!(radius >= 0);
    let min_x = center.x().saturating_sub(radius);
    let max_x = center.x().saturating_add(radius);
    let min_z = center.z().saturating_sub(radius);
    let max_z = center.z().saturating_add(radius);
    let side = usize::try_from(radius.saturating_mul(2).saturating_add(1)).unwrap_or(0);
    let mut positions = Vec::with_capacity(side.saturating_mul(side));

    for x in min_x..=max_x {
        for z in min_z..=max_z {
            positions.push(ChunkCoord::new(x, z));
        }
    }

    positions
}

impl WorldView {
    fn contains(&self, position: ChunkCoord) -> bool {
        view_contains(self.center, self.radius, position)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChunkViewDelta {
    pub(crate) from_center: ChunkCoord,
    pub(crate) to_center: ChunkCoord,
    pub(crate) entering: Vec<ChunkCoord>,
    pub(crate) leaving: Vec<ChunkCoord>,
}

fn chunk_view_delta(
    from_center: ChunkCoord,
    radius: i32,
    to_center: ChunkCoord,
) -> Option<ChunkViewDelta> {
    if from_center == to_center {
        return None;
    }

    let entering = view_positions(to_center, radius)
        .into_iter()
        .filter(|&position| !view_contains(from_center, radius, position))
        .collect();
    let leaving = view_positions(from_center, radius)
        .into_iter()
        .filter(|&position| !view_contains(to_center, radius, position))
        .collect();

    Some(ChunkViewDelta {
        from_center,
        to_center,
        entering,
        leaving,
    })
}

pub(crate) fn plan_view_delta(
    owner: RuntimeId,
    session_id: SessionId,
    to_center: ChunkCoord,
) -> Option<ChunkViewDelta> {
    let views = world_views();
    let view = views.get(&(owner, session_id))?;
    chunk_view_delta(view.center, view.radius, to_center)
}

fn apply_view_delta(view: &mut WorldView, delta: &ChunkViewDelta) -> Result<(), String> {
    if view.center != delta.from_center {
        return Err(
            "pending chunk view delta no longer matches the active world view center".into(),
        );
    }

    let expected = chunk_view_delta(view.center, view.radius, delta.to_center)
        .ok_or_else(|| "pending chunk view delta does not change the active center".to_string())?;
    if expected != *delta {
        return Err("pending chunk view delta does not match the active view geometry".into());
    }

    let mut acquired = Vec::with_capacity(delta.entering.len());
    for &position in &delta.entering {
        if let Err(error) = view.store.pin_chunk(position) {
            for &rollback in &acquired {
                let _ = view.store.unpin_chunk(rollback);
            }
            return Err(error.to_string());
        }
        acquired.push(position);
    }

    view.center = delta.to_center;
    view.pinned_chunks = view_positions(delta.to_center, view.radius);

    for &position in &delta.leaving {
        let _ = view.store.unpin_chunk(position);
    }

    Ok(())
}

pub(crate) fn commit_view_delta(
    owner: RuntimeId,
    session_id: SessionId,
    delta: &ChunkViewDelta,
) -> PhpResult<bool> {
    let mut views = world_views();
    let Some(view) = views.get_mut(&(owner, session_id)) else {
        return Ok(false);
    };

    apply_view_delta(view, delta).map_err(php_error)?;
    Ok(true)
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum ViewChunkQueueResult {
    Sent,
    Backpressured,
    Gone,
}

pub(crate) fn queue_view_delta_chunks(
    owner: RuntimeId,
    session_id: SessionId,
    delta: &ChunkViewDelta,
) -> PhpResult<ViewChunkQueueResult> {
    let (world_handle, center) = {
        let views = world_views();
        let view = views
            .get(&(owner, session_id))
            .ok_or_else(|| php_error("cannot send entering chunks without an active world view"))?;
        (view.world_handle, view.center)
    };

    if center != delta.from_center {
        return Err(php_error(
            "pending chunk view delta no longer matches the active world view center",
        ));
    }
    if delta.entering.is_empty() {
        return Ok(ViewChunkQueueResult::Sent);
    }

    let mut chunks = Vec::with_capacity(delta.entering.len());
    for &position in &delta.entering {
        chunks.push(protocol84_chunk(world_handle, position)?);
    }
    let batch = bootstrap_session_packet(BootstrapPacket::Batch(BatchPacket::new(chunks)))?;

    Ok(
        match try_queue(owner, session_id, batch, SessionDelivery::ReliableOrdered)? {
            QueueResult::Sent => ViewChunkQueueResult::Sent,
            QueueResult::Backpressured => ViewChunkQueueResult::Backpressured,
            QueueResult::Gone => ViewChunkQueueResult::Gone,
        },
    )
}

#[derive(Debug)]
enum PendingChunkSync {
    Blocks(BTreeMap<u16, u16>),
    FullChunk,
}

static WORLD_VIEWS: LazyLock<Mutex<HashMap<(RuntimeId, SessionId), WorldView>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn world_views() -> MutexGuard<'static, HashMap<(RuntimeId, SessionId), WorldView>> {
    match WORLD_VIEWS.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn release_view(view: &WorldView) {
    for &position in &view.pinned_chunks {
        let _ = view.store.unpin_chunk(position);
    }
}

pub(crate) fn forget_session(owner: RuntimeId, session_id: SessionId) {
    if let Some(view) = world_views().remove(&(owner, session_id)) {
        release_view(&view);
    }
}

pub(crate) fn forget_runtime(owner: RuntimeId) {
    let removed = {
        let mut views = world_views();
        let keys = views
            .keys()
            .filter(|(runtime, _)| *runtime == owner)
            .copied()
            .collect::<Vec<_>>();
        keys.into_iter()
            .filter_map(|key| views.remove(&key))
            .collect::<Vec<_>>()
    };

    for view in removed {
        release_view(&view);
    }
}

pub(crate) struct WorldBootstrap {
    pub(crate) seed: i32,
    pub(crate) generator: i32,
    pub(crate) spawn: [i32; 3],
    pub(crate) position: [f32; 3],
    pub(crate) time: i32,
    pub(crate) time_started: bool,
    pub(crate) level_id: String,
}

struct ProjectionReader<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> ProjectionReader<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }

    fn read_exact(&mut self, len: usize) -> PhpResult<&'a [u8]> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| php_error("chunk projection offset overflow"))?;
        if end > self.input.len() {
            return Err(php_error(format!(
                "truncated chunk projection: needed {len} bytes with {} remaining",
                self.input.len().saturating_sub(self.offset)
            )));
        }
        let bytes = &self.input[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }

    fn read_u16_le(&mut self) -> PhpResult<u16> {
        let bytes = self.read_exact(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32_le(&mut self) -> PhpResult<u32> {
        let bytes = self.read_exact(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_i32_le(&mut self) -> PhpResult<i32> {
        let bytes = self.read_exact(4)?;
        Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn finish(self) -> PhpResult<()> {
        if self.offset == self.input.len() {
            Ok(())
        } else {
            Err(php_error(format!(
                "chunk projection has {} trailing bytes",
                self.input.len() - self.offset
            )))
        }
    }
}

pub(crate) fn bootstrap_session_packet(packet: BootstrapPacket) -> PhpResult<SessionPacket> {
    let raw = encode_bootstrap_packet(&packet, codec_limits())
        .map_err(|error| php_error(error.to_string()))?;
    Ok(SessionPacket::new(raw.id(), raw.body().clone()))
}

pub(crate) fn validate_login_body(body: Vec<u8>) -> PhpResult<()> {
    let raw = RawPacket::new(packet_id::LOGIN, NativeBuffer::from_vec(body));
    match decode_bootstrap_packet(raw, codec_limits())
        .map_err(|error| php_error(error.to_string()))?
    {
        BootstrapPacket::Login(_) => Ok(()),
        _ => Err(php_error("expected protocol-84 Login packet")),
    }
}

pub(crate) fn initial_bootstrap_packets(
    bootstrap: &WorldBootstrap,
) -> PhpResult<Vec<SessionPacket>> {
    [
        BootstrapPacket::PlayStatus(PlayStatusPacket::new(PlayStatusPacket::LOGIN_SUCCESS)),
        BootstrapPacket::StartGame(StartGamePacket {
            seed: bootstrap.seed,
            dimension: 0,
            generator: bootstrap.generator,
            gamemode: 0,
            entity_id: 0,
            spawn: bootstrap.spawn,
            position: bootstrap.position,
            level_id: bootstrap.level_id.clone(),
        }),
        BootstrapPacket::SetTime(SetTimePacket::new(bootstrap.time, bootstrap.time_started)),
        BootstrapPacket::SetSpawnPosition(SetSpawnPositionPacket::new(
            bootstrap.spawn[0],
            bootstrap.spawn[1],
            bootstrap.spawn[2],
        )),
        BootstrapPacket::SetDifficulty(SetDifficultyPacket::new(1)),
        BootstrapPacket::AdventureSettings(AdventureSettingsPacket::new(
            AdventureFlags::SURVIVAL.bits() as i32,
            2,
            2,
        )),
    ]
    .into_iter()
    .map(bootstrap_session_packet)
    .collect()
}

pub(crate) fn requested_chunk_radius(body: &[u8]) -> PhpResult<i32> {
    if body.len() != 4 {
        return Err(php_error(format!(
            "protocol-84 RequestChunkRadius body must be exactly 4 bytes, got {}",
            body.len()
        )));
    }

    let radius = i32::from_be_bytes([body[0], body[1], body[2], body[3]]);
    if radius <= 0 {
        return Err(php_error("protocol-84 chunk radius must be positive"));
    }
    Ok(radius)
}

fn i32_field(field: &'static str, value: i64) -> PhpResult<i32> {
    i32::try_from(value).map_err(|_| php_error(format!("{field} must fit signed 32-bit range")))
}

fn decode_initial_chunk_projection(
    input: &[u8],
    expected_chunks: usize,
) -> PhpResult<Vec<RawPacket>> {
    if input.len() > MAX_PROJECTION_BYTES {
        return Err(php_error(format!(
            "initial chunk projection exceeds {MAX_PROJECTION_BYTES} bytes"
        )));
    }

    let mut reader = ProjectionReader::new(input);
    let declared_chunks = usize::try_from(reader.read_u32_le()?)
        .map_err(|_| php_error("initial chunk count exceeds platform size"))?;
    if declared_chunks != expected_chunks {
        return Err(php_error(format!(
            "initial chunk projection count mismatch: expected {expected_chunks}, got {declared_chunks}"
        )));
    }
    if declared_chunks > MAX_INITIAL_CHUNKS {
        return Err(php_error(format!(
            "initial chunk projection exceeds {MAX_INITIAL_CHUNKS} chunks"
        )));
    }

    let mut packets = Vec::with_capacity(declared_chunks);
    for _ in 0..declared_chunks {
        let chunk_x = reader.read_i32_le()?;
        let chunk_z = reader.read_i32_le()?;
        let block_ids = reader.read_exact(CHUNK_BLOCK_COUNT)?;
        let block_data = reader.read_exact(CHUNK_NIBBLE_BYTES)?;
        let sky_light = reader.read_exact(CHUNK_NIBBLE_BYTES)?;
        let block_light = reader.read_exact(CHUNK_NIBBLE_BYTES)?;
        let biomes = reader.read_exact(CHUNK_COLUMN_COUNT)?;
        let height_map = reader.read_exact(CHUNK_COLUMN_COUNT)?;

        let extra_count = usize::try_from(reader.read_u32_le()?)
            .map_err(|_| php_error("chunk extra-data count exceeds platform size"))?;
        if extra_count > CHUNK_BLOCK_COUNT {
            return Err(php_error(
                "chunk extra-data count exceeds fixed-target block count",
            ));
        }
        let mut extra_data = Vec::with_capacity(extra_count);
        for _ in 0..extra_count {
            extra_data.push((reader.read_u32_le()?, reader.read_u16_le()?));
        }

        let snapshot = Protocol84ChunkSnapshot {
            chunk_x,
            chunk_z,
            block_ids,
            block_data,
            sky_light,
            block_light,
            biomes,
            height_map,
            extra_data: &extra_data,
        };
        packets.push(
            encode_protocol84_full_chunk_data(snapshot)
                .map_err(|error| php_error(error.to_string()))?,
        );
    }
    reader.finish()?;
    Ok(packets)
}

pub(crate) fn queue_reliable_ordered(
    owner: RuntimeId,
    session_id: SessionId,
    packets: Vec<SessionPacket>,
) -> PhpResult<()> {
    for packet in packets {
        with_runtime(owner, |host| {
            host.try_send(session_id, packet, SessionDelivery::ReliableOrdered)
        })?;
    }
    Ok(())
}

/// Validates Login and queues protocol-84 bootstrap state projected from the PHP-owned World.
#[php_function]
#[php(name = "cobblestone_session_protocol84_accept_login_world")]
#[allow(clippy::too_many_arguments)]
pub fn cobblestone_session_protocol84_accept_login_world(
    session_id: i64,
    body: Binary<u8>,
    seed: i64,
    generator: i64,
    spawn_x: i64,
    spawn_y: i64,
    spawn_z: i64,
    time: i64,
    time_started: bool,
    level_id: String,
) -> PhpResult<()> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let session_id = owner_session_id(session_id)?;
        validate_login_body(body.into())?;

        let generator = i32_field("generator", generator)?;
        if !(0..=2).contains(&generator) {
            return Err(php_error("protocol-84 generator id must be in range 0..2"));
        }
        let spawn_y = i32_field("spawn y", spawn_y)?;
        if !(0..=127).contains(&spawn_y) {
            return Err(php_error("protocol-84 spawn y must be in range 0..127"));
        }

        let spawn_x = i32_field("spawn x", spawn_x)?;
        let spawn_z = i32_field("spawn z", spawn_z)?;
        let bootstrap = WorldBootstrap {
            seed: i32_field("world seed", seed)?,
            generator,
            spawn: [spawn_x, spawn_y, spawn_z],
            position: [spawn_x as f32 + 0.5, spawn_y as f32, spawn_z as f32 + 0.5],
            time: i32_field("world time", time)?,
            time_started,
            level_id,
        };

        queue_reliable_ordered(owner, session_id, initial_bootstrap_packets(&bootstrap)?)
    })
}

/// Decodes one fixed-target RequestChunkRadius body without changing gameplay/world state.
#[php_function]
#[php(name = "cobblestone_session_protocol84_request_chunk_radius")]
pub fn cobblestone_session_protocol84_request_chunk_radius(body: Binary<u8>) -> PhpResult<i64> {
    php_boundary(|| {
        let _owner = current_runtime_id().map_err(php_error)?;
        let body: Vec<u8> = body.into();
        Ok(i64::from(requested_chunk_radius(&body)?))
    })
}

fn initial_chunk_count(effective_radius: i32) -> PhpResult<usize> {
    if !(1..=MAX_INITIAL_CHUNK_RADIUS).contains(&effective_radius) {
        return Err(php_error(format!(
            "effective initial chunk radius must be in range 1..={MAX_INITIAL_CHUNK_RADIUS}"
        )));
    }

    let side = effective_radius
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| php_error("initial chunk radius overflow"))?;
    usize::try_from(side * side).map_err(|_| php_error("initial chunk count exceeds platform size"))
}

fn queue_initial_chunk_batch(
    owner: RuntimeId,
    session_id: SessionId,
    effective_radius: i32,
    chunks: Vec<RawPacket>,
) -> PhpResult<i64> {
    let expected_chunks = initial_chunk_count(effective_radius)?;
    if chunks.len() != expected_chunks {
        return Err(php_error(format!(
            "initial native chunk count mismatch: expected {expected_chunks}, got {}",
            chunks.len()
        )));
    }

    let batch = bootstrap_session_packet(BootstrapPacket::Batch(BatchPacket::new(chunks)))?;
    let encoded_bytes = batch
        .body()
        .len()
        .checked_add(1)
        .and_then(|value| i64::try_from(value).ok())
        .ok_or_else(|| php_error("encoded chunk Batch length exceeds PHP integer range"))?;

    let packets = vec![
        SessionPacket::new(
            CHUNK_RADIUS_UPDATED_ID,
            NativeBuffer::copy_from_slice(&effective_radius.to_be_bytes()),
        ),
        batch,
        bootstrap_session_packet(BootstrapPacket::PlayStatus(PlayStatusPacket::new(
            PlayStatusPacket::PLAYER_SPAWN,
        )))?,
    ];
    queue_reliable_ordered(owner, session_id, packets)?;
    Ok(encoded_bytes)
}

/// Decodes the private PHP/native bulk projection, encodes exact protocol-84 chunks, and queues
/// ChunkRadiusUpdated + one compressed Batch + PLAYER_SPAWN.
///
/// Returns the compressed Batch packet size (packet id plus body) for owner-runtime observability.
#[php_function]
#[php(name = "cobblestone_session_protocol84_send_initial_chunks")]
pub fn cobblestone_session_protocol84_send_initial_chunks(
    session_id: i64,
    effective_radius: i64,
    projection: Binary<u8>,
) -> PhpResult<i64> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let session_id = owner_session_id(session_id)?;
        let effective_radius = i32_field("effective chunk radius", effective_radius)?;
        let expected_chunks = initial_chunk_count(effective_radius)?;
        let projection: Vec<u8> = projection.into();
        let chunks = decode_initial_chunk_projection(&projection, expected_chunks)?;

        queue_initial_chunk_batch(owner, session_id, effective_radius, chunks)
    })
}

/// Reads immutable native world snapshots directly, reuses revision-keyed protocol-84 chunk
/// packets, and queues ChunkRadiusUpdated + one compressed Batch + PLAYER_SPAWN.
#[php_function]
#[php(name = "cobblestone_session_protocol84_send_native_chunks")]
pub fn cobblestone_session_protocol84_send_native_chunks(
    session_id: i64,
    effective_radius: i64,
    world_handle: i64,
    center_chunk_x: i64,
    center_chunk_z: i64,
) -> PhpResult<i64> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let session_id = owner_session_id(session_id)?;
        let effective_radius = i32_field("effective chunk radius", effective_radius)?;
        let expected_chunks = initial_chunk_count(effective_radius)?;
        let center_x = i32_field("center chunk x", center_chunk_x)?;
        let center_z = i32_field("center chunk z", center_chunk_z)?;
        let min_x = center_x
            .checked_sub(effective_radius)
            .ok_or_else(|| php_error("initial chunk x range underflow"))?;
        let max_x = center_x
            .checked_add(effective_radius)
            .ok_or_else(|| php_error("initial chunk x range overflow"))?;
        let min_z = center_z
            .checked_sub(effective_radius)
            .ok_or_else(|| php_error("initial chunk z range underflow"))?;
        let max_z = center_z
            .checked_add(effective_radius)
            .ok_or_else(|| php_error("initial chunk z range overflow"))?;

        let store = resolve_world(world_handle)?;
        let mut chunks = Vec::with_capacity(expected_chunks);
        let mut positions = Vec::with_capacity(expected_chunks);
        for x in min_x..=max_x {
            for z in min_z..=max_z {
                let position = ChunkCoord::new(x, z);
                chunks.push(protocol84_chunk(world_handle, position)?);
                positions.push(position);
            }
        }

        let mut pinned = Vec::with_capacity(positions.len());
        for &position in &positions {
            if let Err(error) = store.pin_chunk(position) {
                for &rollback in &pinned {
                    let _ = store.unpin_chunk(rollback);
                }
                return Err(php_error(error.to_string()));
            }
            pinned.push(position);
        }

        let encoded = match queue_initial_chunk_batch(owner, session_id, effective_radius, chunks) {
            Ok(encoded) => encoded,
            Err(error) => {
                for &position in &pinned {
                    let _ = store.unpin_chunk(position);
                }
                return Err(error);
            }
        };
        let cursor = store.current_change_sequence();
        let view = WorldView {
            world_handle,
            store: Arc::clone(&store),
            center: ChunkCoord::new(center_x, center_z),
            radius: effective_radius,
            cursor,
            pinned_chunks: positions,
        };
        if let Some(previous) = world_views().insert((owner, session_id), view) {
            release_view(&previous);
        }
        Ok(encoded)
    })
}

fn view_chunks(view: &WorldView) -> Vec<ChunkCoord> {
    view.pinned_chunks.clone()
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

fn update_block_packet(position: ChunkCoord, index: u16, state: u16) -> PhpResult<RawPacket> {
    let local_x = i32::from(index & 0x0f);
    let local_z = i32::from((index >> 4) & 0x0f);
    let y = u8::try_from((index >> 8) & 0x7f)
        .map_err(|_| php_error("world change y does not fit one byte"))?;
    let x = position
        .x()
        .checked_mul(16)
        .and_then(|base| base.checked_add(local_x))
        .ok_or_else(|| php_error("UpdateBlock x coordinate overflow"))?;
    let z = position
        .z()
        .checked_mul(16)
        .and_then(|base| base.checked_add(local_z))
        .ok_or_else(|| php_error("UpdateBlock z coordinate overflow"))?;

    encode_protocol84_update_block(x, y, z, state, UPDATE_BLOCK_FLAG_ALL_PRIORITY)
        .map_err(|error| php_error(error.to_string()))
}

/// Coalesces native world changes and queues bounded reliable ordered Batches per spawned viewer.
///
/// Backpressured viewers keep their previous cursor and retry on a later tick. A viewer that falls
/// behind the bounded world change log is recovered by resending its complete current chunk view.
#[php_function]
#[php(name = "cobblestone_session_protocol84_flush_world_changes")]
pub fn cobblestone_session_protocol84_flush_world_changes(world_handle: i64) -> PhpResult<i64> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let store = resolve_world(world_handle)?;

        let views = {
            let views = world_views();
            views
                .iter()
                .filter_map(|(&(runtime, session_id), view)| {
                    (runtime == owner && view.world_handle == world_handle)
                        .then_some((session_id, view.clone()))
                })
                .collect::<Vec<_>>()
        };

        let latest = store.current_change_sequence();
        if views.is_empty() {
            store.prune_changes_through(latest);
            return Ok(0);
        }
        if views.iter().all(|(_, view)| view.cursor == latest) {
            return Ok(0);
        }

        let log = store.change_log_snapshot();
        let mut cursor_updates = Vec::new();
        let mut gone = Vec::new();
        let mut queued_batches = 0_i64;

        for (session_id, view) in views {
            if view.cursor == log.latest_sequence() {
                continue;
            }

            let mut pending = HashMap::<ChunkCoord, PendingChunkSync>::new();
            if log.cursor_is_stale(view.cursor) {
                for position in view_chunks(&view) {
                    pending.insert(position, PendingChunkSync::FullChunk);
                }
            } else {
                for change in log
                    .changes()
                    .iter()
                    .filter(|change| change.sequence() > view.cursor)
                {
                    if view.contains(change.position()) {
                        merge_change(&mut pending, change.position(), change.kind());
                    }
                }
            }

            if pending.is_empty() {
                cursor_updates.push((session_id, log.latest_sequence()));
                continue;
            }

            let mut chunks = pending.into_iter().collect::<Vec<_>>();
            chunks.sort_unstable_by_key(|(position, _)| (position.x(), position.z()));
            let mut packets = Vec::new();
            for (position, change) in chunks {
                match change {
                    PendingChunkSync::FullChunk => {
                        packets.push(protocol84_chunk(world_handle, position)?);
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
                let batch = bootstrap_session_packet(BootstrapPacket::Batch(BatchPacket::new(
                    packet_batch.to_vec(),
                )))?;
                match try_queue(owner, session_id, batch, SessionDelivery::ReliableOrdered)? {
                    QueueResult::Sent => {
                        queued_batches = queued_batches.saturating_add(1);
                    }
                    QueueResult::Backpressured => {
                        completed = false;
                        break;
                    }
                    QueueResult::Gone => {
                        gone.push(session_id);
                        completed = false;
                        break;
                    }
                }
            }
            if completed {
                cursor_updates.push((session_id, log.latest_sequence()));
            }
        }

        let prune_through = {
            let mut views = world_views();
            for session_id in gone {
                if let Some(view) = views.remove(&(owner, session_id)) {
                    release_view(&view);
                }
            }
            for (session_id, cursor) in cursor_updates {
                if let Some(view) = views.get_mut(&(owner, session_id))
                    && view.world_handle == world_handle
                {
                    view.cursor = cursor;
                }
            }

            views
                .iter()
                .filter_map(|(&(runtime, _), view)| {
                    (runtime == owner && view.world_handle == world_handle).then_some(view.cursor)
                })
                .min()
                .unwrap_or(log.latest_sequence())
        };
        store.prune_changes_through(prune_through);

        Ok(queued_batches)
    })
}

pub(crate) fn register(module: ModuleBuilder) -> ModuleBuilder {
    module
        .function(wrap_function!(
            cobblestone_session_protocol84_accept_login_world
        ))
        .function(wrap_function!(
            cobblestone_session_protocol84_request_chunk_radius
        ))
        .function(wrap_function!(
            cobblestone_session_protocol84_send_initial_chunks
        ))
        .function(wrap_function!(
            cobblestone_session_protocol84_send_native_chunks
        ))
        .function(wrap_function!(
            cobblestone_session_protocol84_flush_world_changes
        ))
}

#[cfg(test)]
mod view_delta_tests {
    use super::*;

    fn positions(values: &[(i32, i32)]) -> Vec<ChunkCoord> {
        values.iter().map(|&(x, z)| ChunkCoord::new(x, z)).collect()
    }

    fn pinned_view(center: ChunkCoord, radius: i32) -> WorldView {
        let store = Arc::new(WorldStore::new());
        let pinned_chunks = view_positions(center, radius);
        for &position in &pinned_chunks {
            store.ensure_chunk(position, 1);
            store.pin_chunk(position).expect("pin initial view chunk");
        }

        WorldView {
            world_handle: 1,
            store,
            center,
            radius,
            cursor: 7,
            pinned_chunks,
        }
    }

    #[test]
    fn unchanged_center_has_no_delta() {
        assert_eq!(
            chunk_view_delta(ChunkCoord::new(8, 8), 2, ChunkCoord::new(8, 8)),
            None
        );
    }

    #[test]
    fn cardinal_shift_has_deterministic_entering_and_leaving_edges() {
        let delta =
            chunk_view_delta(ChunkCoord::new(0, 0), 1, ChunkCoord::new(1, 0)).expect("delta");

        assert_eq!(delta.from_center, ChunkCoord::new(0, 0));
        assert_eq!(delta.to_center, ChunkCoord::new(1, 0));
        assert_eq!(delta.entering, positions(&[(2, -1), (2, 0), (2, 1)]));
        assert_eq!(delta.leaving, positions(&[(-1, -1), (-1, 0), (-1, 1)]));
    }

    #[test]
    fn diagonal_shift_keeps_x_then_z_order() {
        let delta =
            chunk_view_delta(ChunkCoord::new(0, 0), 1, ChunkCoord::new(1, 1)).expect("delta");

        assert_eq!(
            delta.entering,
            positions(&[(0, 2), (1, 2), (2, 0), (2, 1), (2, 2)])
        );
        assert_eq!(
            delta.leaving,
            positions(&[(-1, -1), (-1, 0), (-1, 1), (0, -1), (1, -1)])
        );
    }

    #[test]
    fn multi_chunk_jump_replaces_the_entire_disjoint_view() {
        let delta =
            chunk_view_delta(ChunkCoord::new(0, 0), 1, ChunkCoord::new(4, 0)).expect("delta");

        assert_eq!(delta.entering.len(), 9);
        assert_eq!(delta.leaving.len(), 9);
        assert_eq!(delta.entering.first(), Some(&ChunkCoord::new(3, -1)));
        assert_eq!(delta.entering.last(), Some(&ChunkCoord::new(5, 1)));
        assert_eq!(delta.leaving.first(), Some(&ChunkCoord::new(-1, -1)));
        assert_eq!(delta.leaving.last(), Some(&ChunkCoord::new(1, 1)));
        assert!(
            delta
                .entering
                .iter()
                .all(|position| !delta.leaving.contains(position))
        );
    }

    #[test]
    fn applying_delta_transfers_native_pins_and_preserves_cursor() {
        let mut view = pinned_view(ChunkCoord::new(0, 0), 1);
        let delta =
            chunk_view_delta(view.center, view.radius, ChunkCoord::new(1, 0)).expect("delta");
        for &position in &delta.entering {
            view.store.ensure_chunk(position, 1);
        }

        let shared_leaving = delta.leaving[0];
        assert_eq!(view.store.pin_chunk(shared_leaving).unwrap(), 2);

        apply_view_delta(&mut view, &delta).expect("apply view delta");

        assert_eq!(view.center, ChunkCoord::new(1, 0));
        assert_eq!(view.cursor, 7);
        assert_eq!(view.pinned_chunks, view_positions(ChunkCoord::new(1, 0), 1));
        for &position in &delta.entering {
            assert_eq!(view.store.pin_count(position).unwrap(), 1);
        }
        for &position in &delta.leaving[1..] {
            assert_eq!(view.store.pin_count(position).unwrap(), 0);
        }
        assert_eq!(view.store.pin_count(shared_leaving).unwrap(), 1);
        assert_eq!(view.store.pin_count(ChunkCoord::new(0, 0)).unwrap(), 1);
    }

    #[test]
    fn failed_entry_pin_rolls_back_without_publishing_new_view() {
        let mut view = pinned_view(ChunkCoord::new(0, 0), 1);
        let original_positions = view.pinned_chunks.clone();
        let delta =
            chunk_view_delta(view.center, view.radius, ChunkCoord::new(1, 0)).expect("delta");

        for &position in &delta.entering[..2] {
            view.store.ensure_chunk(position, 1);
        }

        assert!(apply_view_delta(&mut view, &delta).is_err());
        assert_eq!(view.center, ChunkCoord::new(0, 0));
        assert_eq!(view.pinned_chunks, original_positions);
        for &position in &delta.entering[..2] {
            assert_eq!(view.store.pin_count(position).unwrap(), 0);
        }
        for &position in &delta.leaving {
            assert_eq!(view.store.pin_count(position).unwrap(), 1);
        }
    }
}
