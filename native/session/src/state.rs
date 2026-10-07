use std::collections::VecDeque;
use std::sync::Arc;

use bytes::Bytes;
use cobblestone_target::ChunkShape;
use cobblestone_wire::{
    AdventureFlags, AdventureSettingsPacket, BatchPacket, BootstrapPacket, CodecLimits,
    DimensionId, PlayStatusPacket, RawPacket, SetDifficultyPacket, SetSpawnPositionPacket,
    SetTimePacket, StartGamePacket, decode_bootstrap_packet, decode_move_player,
    encode_bootstrap_packet, packet_id,
};
use cobblestone_world::{ChunkCoord, WorldStore};

use crate::view::{
    ChunkViewTransition, PreparedChunks, WorldView, chunk_view_transition, initial_send_positions,
    prioritize_for_player, view_positions,
};
use crate::{SessionId, SessionPacket};

const MAX_DEFERRED_PACKETS: usize = 16;
const MAX_PLAYER_COORDINATE: f32 = 1_000_000.0;

/// World/session values required to accept one fixed-target Login.
#[derive(Debug, Clone)]
pub struct SessionWorldBootstrap {
    seed: i32,
    dimension: DimensionId,
    generator: i32,
    spawn: [i32; 3],
    position: [f32; 3],
    time: i32,
    time_started: bool,
    level_id: String,
}

impl SessionWorldBootstrap {
    /// Validates and creates one fixed-target bootstrap projection.
    pub fn new(
        seed: i32,
        dimension: DimensionId,
        generator: i32,
        spawn: [i32; 3],
        time: i32,
        time_started: bool,
        level_id: String,
    ) -> Result<Self, String> {
        if !(0..=2).contains(&generator) {
            return Err("fixed-target generator id must be in range 0..2".into());
        }
        if !(0..ChunkShape::HEIGHT as i32).contains(&spawn[1]) {
            return Err(format!(
                "fixed-target spawn y must be in range 0..{}",
                ChunkShape::HEIGHT - 1
            ));
        }
        let position = [
            spawn[0] as f32 + 0.5,
            spawn[1] as f32,
            spawn[2] as f32 + 0.5,
        ];
        validate_position(position)?;

        Ok(Self {
            seed,
            dimension,
            generator,
            spawn,
            position,
            time,
            time_started,
            level_id,
        })
    }
}

/// Kind of concrete world preparation currently required by one session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkWorkKind {
    /// Initial spawn view preparation.
    Initial,
    /// Post-spawn view transition preparation.
    View,
}

/// Snapshot of concrete chunks the PHP world owner must make ready for one native session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkWork {
    session_id: SessionId,
    kind: ChunkWorkKind,
    positions: Vec<ChunkCoord>,
    send_positions: Vec<ChunkCoord>,
}

impl ChunkWork {
    /// Session that owns this pending work.
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Whether this work is the initial view or a post-spawn transition.
    #[must_use]
    pub const fn kind(&self) -> ChunkWorkKind {
        self.kind
    }

    /// Chunks that are not yet pinned/prepared by the native session state.
    #[must_use]
    pub fn positions(&self) -> &[ChunkCoord] {
        &self.positions
    }

    /// Full chunk order that must be encoded when the pending work completes.
    #[must_use]
    pub fn send_positions(&self) -> &[ChunkCoord] {
        &self.send_positions
    }
}

/// Metrics produced when an initial view is accepted into the native send queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitialViewResult {
    requested_radius: i32,
    effective_radius: i32,
    chunks_sent: usize,
    encoded_bytes: usize,
}

impl InitialViewResult {
    /// Client-requested radius before the server cap.
    #[must_use]
    pub const fn requested_radius(self) -> i32 {
        self.requested_radius
    }

    /// Effective radius after the server cap.
    #[must_use]
    pub const fn effective_radius(self) -> i32 {
        self.effective_radius
    }

    /// Full chunks included in the initial view.
    #[must_use]
    pub const fn chunks_sent(self) -> usize {
        self.chunks_sent
    }

    /// Encoded compressed Batch packet size, including packet id.
    #[must_use]
    pub const fn encoded_bytes(self) -> usize {
        self.encoded_bytes
    }
}

/// Result of attempting to queue and commit one fully prepared chunk-work item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkWorkCompletion {
    /// Initial view was queued and the session entered spawned/gameplay state.
    Spawned(InitialViewResult),
    /// Post-spawn view transition was queued and committed.
    Complete,
    /// Bounded session command queue was full; the prepared work remains retryable.
    Backpressured,
    /// The session no longer accepts commands.
    Gone,
}

/// Immutable active world-view snapshot used by world-change synchronization.
#[derive(Clone)]
pub struct WorldViewSnapshot {
    session_id: SessionId,
    store: Arc<WorldStore>,
    center: ChunkCoord,
    radius: i32,
    cursor: u64,
    pinned_chunks: Vec<ChunkCoord>,
}

impl WorldViewSnapshot {
    /// Session that owns the view.
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Native world store backing the view.
    #[must_use]
    pub fn store(&self) -> &Arc<WorldStore> {
        &self.store
    }

    /// Current change-log cursor.
    #[must_use]
    pub const fn cursor(&self) -> u64 {
        self.cursor
    }

    /// Current pinned chunks.
    #[must_use]
    pub fn pinned_chunks(&self) -> &[ChunkCoord] {
        &self.pinned_chunks
    }

    /// Returns whether a chunk is inside the current view geometry.
    #[must_use]
    pub fn contains(&self, position: ChunkCoord) -> bool {
        crate::view::view_contains(self.center, self.radius, position)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JoinPhase {
    WaitingLogin,
    LoginPending,
    WaitingChunkRadius,
    WaitingInitialChunks,
    Spawned,
}

pub(super) enum InboundAction {
    LoginRequested,
    Forward(SessionPacket),
    Consumed,
}

enum PendingWork {
    Initial {
        requested_radius: i32,
        effective_radius: i32,
        center: ChunkCoord,
        prepare_positions: Vec<ChunkCoord>,
        send_positions: Vec<ChunkCoord>,
        prepared: PreparedChunks,
    },
    View {
        transition: ChunkViewTransition,
        prepared: PreparedChunks,
    },
}

impl PendingWork {
    fn kind(&self) -> ChunkWorkKind {
        match self {
            Self::Initial { .. } => ChunkWorkKind::Initial,
            Self::View { .. } => ChunkWorkKind::View,
        }
    }

    fn prepare_positions(&self) -> &[ChunkCoord] {
        match self {
            Self::Initial {
                prepare_positions, ..
            } => prepare_positions,
            Self::View { transition, .. } => &transition.entering,
        }
    }

    fn send_positions(&self) -> &[ChunkCoord] {
        match self {
            Self::Initial { send_positions, .. } => send_positions,
            Self::View { transition, .. } => &transition.entering,
        }
    }

    fn prepared(&self) -> &PreparedChunks {
        match self {
            Self::Initial { prepared, .. } | Self::View { prepared, .. } => prepared,
        }
    }

    fn prepared_mut(&mut self) -> &mut PreparedChunks {
        match self {
            Self::Initial { prepared, .. } | Self::View { prepared, .. } => prepared,
        }
    }
}

pub(super) struct CompletionPlan {
    pub(super) packets: Vec<SessionPacket>,
    pub(super) initial: Option<InitialViewResult>,
}

pub(super) struct SessionState {
    phase: JoinPhase,
    max_chunk_radius: i32,
    position: Option<[f32; 3]>,
    player_chunk: Option<ChunkCoord>,
    desired_radius: Option<i32>,
    view: Option<WorldView>,
    pending: Option<PendingWork>,
    deferred: VecDeque<SessionPacket>,
}

impl SessionState {
    pub(super) fn new(max_chunk_radius: i32) -> Self {
        Self {
            phase: JoinPhase::WaitingLogin,
            max_chunk_radius,
            position: None,
            player_chunk: None,
            desired_radius: None,
            view: None,
            pending: None,
            deferred: VecDeque::new(),
        }
    }

    pub(super) fn handle_packet(
        &mut self,
        packet: SessionPacket,
        limits: CodecLimits,
    ) -> Result<InboundAction, String> {
        match self.phase {
            JoinPhase::WaitingLogin => {
                if packet.id() != packet_id::LOGIN {
                    return Err(format!(
                        "expected Login, got fixed-target packet {}",
                        packet.id()
                    ));
                }
                validate_login(packet.body(), limits)?;
                self.phase = JoinPhase::LoginPending;
                Ok(InboundAction::LoginRequested)
            }
            JoinPhase::LoginPending => {
                if self.deferred.len() >= MAX_DEFERRED_PACKETS {
                    return Err(
                        "too many packets arrived while Login acceptance was pending".into(),
                    );
                }
                self.deferred.push_back(packet);
                Ok(InboundAction::Consumed)
            }
            JoinPhase::WaitingChunkRadius => {
                if packet.id() != packet_id::REQUEST_CHUNK_RADIUS {
                    return Err(format!(
                        "expected RequestChunkRadius, got fixed-target packet {}",
                        packet.id()
                    ));
                }
                let requested_radius = requested_chunk_radius(packet.body())?;
                let effective_radius = requested_radius.min(self.max_chunk_radius);
                let center = self
                    .player_chunk
                    .ok_or_else(|| "accepted Login lost player spawn chunk".to_owned())?;
                let player = self
                    .position
                    .ok_or_else(|| "accepted Login lost player spawn position".to_owned())?;
                let mut prepare_positions = view_positions(center, effective_radius);
                prioritize_for_player(&mut prepare_positions, player);
                let send_positions = initial_send_positions(center, effective_radius);
                self.pending = Some(PendingWork::Initial {
                    requested_radius,
                    effective_radius,
                    center,
                    prepare_positions,
                    send_positions,
                    prepared: PreparedChunks::new(),
                });
                self.phase = JoinPhase::WaitingInitialChunks;
                Ok(InboundAction::Consumed)
            }
            JoinPhase::WaitingInitialChunks => {
                if packet.id() == packet_id::REQUEST_CHUNK_RADIUS {
                    return Ok(InboundAction::Consumed);
                }
                Err(format!(
                    "initial chunks are still loading; unexpected fixed-target packet {}",
                    packet.id()
                ))
            }
            JoinPhase::Spawned => {
                if packet.id() == packet_id::MOVE_PLAYER {
                    let movement =
                        decode_move_player(packet.body()).map_err(|error| error.to_string())?;
                    let position = movement.position();
                    let chunk = validate_position(position)?;
                    self.position = Some(position);
                    self.player_chunk = Some(chunk);
                    let radius = self
                        .desired_radius
                        .or_else(|| self.view.as_ref().map(|view| view.radius))
                        .ok_or_else(|| "spawned session has no active chunk radius".to_owned())?;
                    self.plan_view_transition(chunk, radius, position)?;
                } else if packet.id() == packet_id::REQUEST_CHUNK_RADIUS {
                    let requested = requested_chunk_radius(packet.body())?;
                    let effective = requested.min(self.max_chunk_radius);
                    self.desired_radius = Some(effective);
                    let chunk = self
                        .player_chunk
                        .ok_or_else(|| "spawned session has no player chunk".to_owned())?;
                    let position = self
                        .position
                        .ok_or_else(|| "spawned session has no player position".to_owned())?;
                    self.plan_view_transition(chunk, effective, position)?;
                }
                Ok(InboundAction::Forward(packet))
            }
        }
    }

    pub(super) fn take_deferred(&mut self) -> Option<SessionPacket> {
        if self.phase == JoinPhase::LoginPending {
            None
        } else {
            self.deferred.pop_front()
        }
    }

    pub(super) fn accept_login(
        &mut self,
        bootstrap: SessionWorldBootstrap,
        limits: CodecLimits,
    ) -> Result<Vec<SessionPacket>, String> {
        if self.phase != JoinPhase::LoginPending {
            return Err("session is not awaiting Login acceptance".into());
        }
        let packets = bootstrap_packets(&bootstrap, limits)?;
        self.position = Some(bootstrap.position);
        self.player_chunk = Some(position_chunk(bootstrap.position));
        self.phase = JoinPhase::WaitingChunkRadius;
        Ok(packets)
    }

    pub(super) fn rollback_login_acceptance(&mut self) {
        if self.phase == JoinPhase::WaitingChunkRadius
            && self.pending.is_none()
            && self.view.is_none()
        {
            self.phase = JoinPhase::LoginPending;
            self.position = None;
            self.player_chunk = None;
        }
    }

    pub(super) fn chunk_work(&self, session_id: SessionId) -> Option<ChunkWork> {
        let pending = self.pending.as_ref()?;
        Some(ChunkWork {
            session_id,
            kind: pending.kind(),
            positions: pending.prepared().unprepared(pending.prepare_positions()),
            send_positions: pending.send_positions().to_vec(),
        })
    }

    pub(super) fn mark_prepared_chunk(
        &mut self,
        store: Arc<WorldStore>,
        position: ChunkCoord,
    ) -> Result<(), String> {
        let pending = self
            .pending
            .as_mut()
            .ok_or_else(|| "session has no pending chunk work".to_owned())?;
        let expected = pending.prepare_positions().to_vec();
        if let Some(view) = &self.view
            && !Arc::ptr_eq(&view.store, &store)
        {
            return Err("pending session view targets a different world store".into());
        }
        pending.prepared_mut().mark(&expected, store, position)
    }

    pub(super) fn completion_plan(
        &self,
        store: &Arc<WorldStore>,
        raw_chunks: Vec<RawPacket>,
        limits: CodecLimits,
    ) -> Result<CompletionPlan, String> {
        let pending = self
            .pending
            .as_ref()
            .ok_or_else(|| "session has no pending chunk work".to_owned())?;
        if !pending.prepared().is_complete(pending.prepare_positions()) {
            return Err("session chunk work is not fully prepared".into());
        }
        if !pending.prepared().matches_store(store) {
            return Err("session chunk work belongs to a different world store".into());
        }
        if let Some(view) = &self.view
            && !Arc::ptr_eq(&view.store, store)
        {
            return Err("session active view belongs to a different world store".into());
        }
        if raw_chunks.len() != pending.send_positions().len() {
            return Err(format!(
                "prepared chunk packet count mismatch: expected {}, got {}",
                pending.send_positions().len(),
                raw_chunks.len()
            ));
        }

        match pending {
            PendingWork::Initial {
                requested_radius,
                effective_radius,
                send_positions,
                ..
            } => {
                let batch = bootstrap_session_packet(
                    BootstrapPacket::Batch(BatchPacket::new(raw_chunks)),
                    limits,
                )?;
                let encoded_bytes = batch
                    .body()
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| "encoded initial chunk Batch length overflow".to_owned())?;
                let packets = vec![
                    SessionPacket::new(
                        packet_id::CHUNK_RADIUS_UPDATED,
                        Bytes::copy_from_slice(&effective_radius.to_be_bytes()),
                    ),
                    batch,
                    bootstrap_session_packet(
                        BootstrapPacket::PlayStatus(PlayStatusPacket::new(
                            PlayStatusPacket::PLAYER_SPAWN,
                        )),
                        limits,
                    )?,
                ];
                Ok(CompletionPlan {
                    packets,
                    initial: Some(InitialViewResult {
                        requested_radius: *requested_radius,
                        effective_radius: *effective_radius,
                        chunks_sent: send_positions.len(),
                        encoded_bytes,
                    }),
                })
            }
            PendingWork::View { transition, .. } => {
                let mut packets = Vec::with_capacity(
                    raw_chunks.len() + usize::from(transition.from_radius != transition.to_radius),
                );
                if transition.from_radius != transition.to_radius {
                    packets.push(RawPacket::new(
                        packet_id::CHUNK_RADIUS_UPDATED,
                        Bytes::copy_from_slice(&transition.to_radius.to_be_bytes()),
                    ));
                }
                packets.extend(raw_chunks);
                let batch = bootstrap_session_packet(
                    BootstrapPacket::Batch(BatchPacket::new(packets)),
                    limits,
                )?;
                Ok(CompletionPlan {
                    packets: vec![batch],
                    initial: None,
                })
            }
        }
    }

    pub(super) fn commit_pending(&mut self) -> Result<Option<InitialViewResult>, String> {
        let pending = self
            .pending
            .take()
            .ok_or_else(|| "session has no pending chunk work".to_owned())?;

        match pending {
            PendingWork::Initial {
                requested_radius,
                effective_radius,
                center,
                send_positions,
                mut prepared,
                ..
            } => {
                if !prepared.is_complete(&send_positions) {
                    // Prepare order and send order contain the same set; the stricter set check above
                    // catches accidental divergence before native ownership is published.
                    let prepare = view_positions(center, effective_radius);
                    if !prepared.is_complete(&prepare) {
                        return Err("initial chunk work lost prepared pins before commit".into());
                    }
                }
                let store = prepared
                    .take_store()
                    .ok_or_else(|| "initial chunk work has no native world store".to_owned())?;
                prepared.disarm();
                let cursor = store.current_change_sequence();
                self.view = Some(WorldView {
                    store,
                    center,
                    radius: effective_radius,
                    cursor,
                    pinned_chunks: view_positions(center, effective_radius),
                });
                self.phase = JoinPhase::Spawned;
                self.desired_radius = None;
                Ok(Some(InitialViewResult {
                    requested_radius,
                    effective_radius,
                    chunks_sent: send_positions.len(),
                    encoded_bytes: 0,
                }))
            }
            PendingWork::View {
                transition,
                mut prepared,
            } => {
                let view = self.view.as_mut().ok_or_else(|| {
                    "pending view transition lost its active world view".to_owned()
                })?;
                if view.center != transition.from_center || view.radius != transition.from_radius {
                    return Err("pending view transition no longer matches the active view".into());
                }
                if !prepared.matches_store(&view.store) {
                    return Err(
                        "prepared view transition belongs to a different world store".into(),
                    );
                }
                if !prepared.is_complete(&transition.entering) {
                    return Err("view transition lost prepared entering pins before commit".into());
                }
                prepared.disarm();
                for &position in &transition.leaving {
                    let _ = view.store.unpin_chunk(position);
                }
                view.center = transition.to_center;
                view.radius = transition.to_radius;
                view.pinned_chunks = view_positions(transition.to_center, transition.to_radius);
                self.desired_radius = None;
                Ok(None)
            }
        }
    }

    pub(super) fn view_snapshot(&self, session_id: SessionId) -> Option<WorldViewSnapshot> {
        let view = self.view.as_ref()?;
        Some(WorldViewSnapshot {
            session_id,
            store: Arc::clone(&view.store),
            center: view.center,
            radius: view.radius,
            cursor: view.cursor,
            pinned_chunks: view.pinned_chunks.clone(),
        })
    }

    pub(super) fn update_view_cursor(&mut self, store: &Arc<WorldStore>, cursor: u64) -> bool {
        let Some(view) = self.view.as_mut() else {
            return false;
        };
        if !Arc::ptr_eq(&view.store, store) {
            return false;
        }
        view.cursor = cursor;
        true
    }

    fn plan_view_transition(
        &mut self,
        to_center: ChunkCoord,
        to_radius: i32,
        player: [f32; 3],
    ) -> Result<(), String> {
        let view = self
            .view
            .as_ref()
            .ok_or_else(|| "cannot plan chunk view without an active world view".to_owned())?;
        let mut transition =
            match chunk_view_transition(view.center, view.radius, to_center, to_radius) {
                Some(transition) => transition,
                None => {
                    self.pending = None;
                    self.desired_radius = None;
                    return Ok(());
                }
            };
        prioritize_for_player(&mut transition.entering, player);

        let same = matches!(
            self.pending.as_ref(),
            Some(PendingWork::View { transition: current, .. }) if
                current.from_center == transition.from_center
                    && current.from_radius == transition.from_radius
                    && current.to_center == transition.to_center
                    && current.to_radius == transition.to_radius
        );
        if !same {
            self.pending = Some(PendingWork::View {
                transition,
                prepared: PreparedChunks::new(),
            });
        }
        Ok(())
    }
}

fn validate_login(body: &Bytes, limits: CodecLimits) -> Result<(), String> {
    let raw = RawPacket::new(packet_id::LOGIN, body.clone());
    match decode_bootstrap_packet(raw, limits).map_err(|error| error.to_string())? {
        BootstrapPacket::Login(_) => Ok(()),
        _ => Err("expected fixed-target Login packet".into()),
    }
}

fn requested_chunk_radius(body: &Bytes) -> Result<i32, String> {
    if body.len() != 4 {
        return Err(format!(
            "fixed-target RequestChunkRadius body must be exactly 4 bytes, got {}",
            body.len()
        ));
    }
    let radius = i32::from_be_bytes([body[0], body[1], body[2], body[3]]);
    if radius <= 0 {
        return Err("fixed-target chunk radius must be positive".into());
    }
    Ok(radius)
}

fn validate_position(position: [f32; 3]) -> Result<ChunkCoord, String> {
    if position
        .iter()
        .any(|coordinate| !coordinate.is_finite() || coordinate.abs() > MAX_PLAYER_COORDINATE)
    {
        return Err(
            "fixed-target MovePlayer position is non-finite or outside the supported world range"
                .into(),
        );
    }
    Ok(position_chunk(position))
}

fn position_chunk(position: [f32; 3]) -> ChunkCoord {
    ChunkCoord::new(
        (position[0] / ChunkShape::EDGE as f32).floor() as i32,
        (position[2] / ChunkShape::EDGE as f32).floor() as i32,
    )
}

fn bootstrap_session_packet(
    packet: BootstrapPacket,
    limits: CodecLimits,
) -> Result<SessionPacket, String> {
    let raw = encode_bootstrap_packet(&packet, limits).map_err(|error| error.to_string())?;
    Ok(SessionPacket::new(raw.id(), raw.body().clone()))
}

fn bootstrap_packets(
    bootstrap: &SessionWorldBootstrap,
    limits: CodecLimits,
) -> Result<Vec<SessionPacket>, String> {
    [
        BootstrapPacket::PlayStatus(PlayStatusPacket::new(PlayStatusPacket::LOGIN_SUCCESS)),
        BootstrapPacket::StartGame(StartGamePacket {
            seed: bootstrap.seed,
            dimension: bootstrap.dimension,
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
    .map(|packet| bootstrap_session_packet(packet, limits))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobblestone_wire::{FULL_CHUNK_DATA_ID, LoginPacket};

    fn limits() -> CodecLimits {
        CodecLimits::new(4096, 4096, 4096, 4096, 2048, 32)
    }

    fn login_packet() -> SessionPacket {
        let raw = encode_bootstrap_packet(
            &BootstrapPacket::Login(LoginPacket::new(
                Bytes::copy_from_slice(b"{}"),
                Bytes::copy_from_slice(b"test-skin"),
            )),
            limits(),
        )
        .expect("encode login");
        SessionPacket::new(raw.id(), raw.body().clone())
    }

    fn initial_pending_state() -> (SessionState, Arc<WorldStore>, Vec<ChunkCoord>) {
        let mut state = SessionState::new(3);
        assert!(matches!(
            state
                .handle_packet(login_packet(), limits())
                .expect("login"),
            InboundAction::LoginRequested
        ));
        state
            .accept_login(
                SessionWorldBootstrap::new(
                    1234,
                    DimensionId::Overworld,
                    1,
                    [0, 64, 0],
                    0,
                    true,
                    "State Test".to_owned(),
                )
                .expect("bootstrap"),
                limits(),
            )
            .expect("accept login");
        state
            .handle_packet(
                SessionPacket::new(
                    packet_id::REQUEST_CHUNK_RADIUS,
                    Bytes::copy_from_slice(&1_i32.to_be_bytes()),
                ),
                limits(),
            )
            .expect("radius");

        let session_id = SessionId::new(1).expect("session id");
        let work = state.chunk_work(session_id).expect("chunk work");
        let positions = work.positions().to_vec();
        let store = Arc::new(WorldStore::new());
        for &position in &positions {
            store.ensure_chunk(position, 1).expect("resident chunk");
            state
                .mark_prepared_chunk(Arc::clone(&store), position)
                .expect("prepare pin");
        }
        (state, store, positions)
    }

    #[test]
    fn uncommitted_completion_keeps_prepared_pins_retryable() {
        let (mut state, store, positions) = initial_pending_state();
        let session_id = SessionId::new(1).expect("session id");
        let work = state.chunk_work(session_id).expect("chunk work");
        let packets = work
            .send_positions()
            .iter()
            .map(|_| RawPacket::new(FULL_CHUNK_DATA_ID, Bytes::new()))
            .collect();

        let plan = state
            .completion_plan(&store, packets, limits())
            .expect("completion plan");
        assert!(plan.initial.is_some());
        drop(plan); // Models command-queue backpressure: no commit is published.

        assert!(state.chunk_work(session_id).is_some());
        for &position in &positions {
            assert_eq!(store.pin_count(position).expect("pin count"), 1);
        }

        state.commit_pending().expect("retry commit");
        for &position in &positions {
            assert_eq!(store.pin_count(position).expect("pin count"), 1);
        }
        drop(state);
        for position in positions {
            assert_eq!(store.pin_count(position).expect("pin count"), 0);
        }
    }
}
