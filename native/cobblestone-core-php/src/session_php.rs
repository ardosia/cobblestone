use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::sync::{Mutex, MutexGuard};

use cobblestone_codec::{
    AdventureFlags, AdventureSettingsPacket, BatchPacket, BootstrapPacket, CodecLimits,
    PlayStatusPacket, RawPacket, SetDifficultyPacket, SetSpawnPositionPacket, SetTimePacket,
    StartGamePacket, decode_bootstrap_frame, decode_game_frame, encode_bootstrap_frame,
    encode_game_frame, packet_id,
};
use cobblestone_core::{NativeBuffer, RuntimeId};
use cobblestone_network::NetworkConfig;
use cobblestone_session::{
    SessionDelivery, SessionHost, SessionHostConfig, SessionHostError, SessionHostEvent, SessionId,
    SessionPacket,
};
use ext_php_rs::binary::Binary;
use ext_php_rs::convert::IntoZval;
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;
use ext_php_rs::types::Zval;

use super::{current_runtime_id, php_boundary, php_error};

const EVENT_QUEUE_CAPACITY: usize = 4096;
const SESSION_COMMAND_CAPACITY: usize = 256;
const PROBE_CHUNK_RADIUS: i32 = 2;
const FULL_CHUNK_DATA_ID: u8 = 0x34;
const CHUNK_RADIUS_UPDATED_ID: u8 = 0x3e;
const CHUNK_ORDER_LAYERED: u8 = 1;

static SESSION_RUNTIME: Mutex<Option<PhpSessionRuntime>> = Mutex::new(None);

struct PhpSessionRuntime {
    owner: RuntimeId,
    host: SessionHost,
}

fn session_runtime() -> MutexGuard<'static, Option<PhpSessionRuntime>> {
    match SESSION_RUNTIME.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn codec_limits() -> CodecLimits {
    CodecLimits::new(
        2 * 1024 * 1024,
        2 * 1024 * 1024,
        2 * 1024 * 1024,
        4 * 1024 * 1024,
        2 * 1024 * 1024,
        256,
    )
}

fn owner_session_id(value: i64) -> PhpResult<SessionId> {
    let raw = u64::try_from(value).map_err(|_| php_error("session id must be positive"))?;
    SessionId::new(raw).ok_or_else(|| php_error("session id must be nonzero"))
}

fn delivery(value: i64) -> PhpResult<SessionDelivery> {
    match value {
        0 => Ok(SessionDelivery::Unreliable),
        1 => Ok(SessionDelivery::UnreliableSequenced),
        2 => Ok(SessionDelivery::Reliable),
        3 => Ok(SessionDelivery::ReliableOrdered),
        4 => Ok(SessionDelivery::ReliableSequenced),
        _ => Err(php_error("invalid Cobblestone session delivery mode")),
    }
}

fn with_runtime<T>(
    owner: RuntimeId,
    operation: impl FnOnce(&SessionHost) -> Result<T, SessionHostError>,
) -> PhpResult<T> {
    let state = session_runtime();
    let runtime = state
        .as_ref()
        .ok_or_else(|| php_error("Cobblestone session runtime is not started"))?;
    if runtime.owner != owner {
        return Err(php_error(
            "Cobblestone session runtime belongs to another PHP runtime",
        ));
    }
    operation(&runtime.host).map_err(|error| php_error(error.to_string()))
}

fn zval<T: IntoZval>(value: T) -> PhpResult<Zval> {
    value
        .into_zval(false)
        .map_err(|error| php_error(error.to_string()))
}

fn event_values(event: SessionHostEvent) -> PhpResult<Vec<Zval>> {
    match event {
        SessionHostEvent::Connected { session_id, peer } => Ok(vec![
            zval("connected".to_owned())?,
            zval(session_id.get())?,
            zval(peer)?,
        ]),
        SessionHostEvent::Packet { session_id, packet } => Ok(vec![
            zval("packet".to_owned())?,
            zval(session_id.get())?,
            zval(i64::from(packet.id()))?,
            zval(Binary::new(packet.body().as_slice().to_vec()))?,
        ]),
        SessionHostEvent::Disconnected { session_id, reason } => Ok(vec![
            zval("disconnected".to_owned())?,
            zval(session_id.get())?,
            zval(reason)?,
        ]),
    }
}

/// Starts the fixed-target native session mechanism for the current owning PHP runtime.
///
/// This is an internal kernel bridge, not a plugin API.
#[php_function]
pub fn cobblestone_session_start(
    bind: String,
    max_connections: i64,
    server_name: String,
) -> PhpResult<()> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        if server_name.is_empty()
            || server_name.len() > 64
            || server_name.contains(';')
            || server_name.contains('\\')
        {
            return Err(php_error(
                "server name must be 1..64 bytes and contain no semicolon or backslash",
            ));
        }

        let bind_addr = bind
            .parse::<SocketAddr>()
            .map_err(|error| php_error(format!("invalid session bind address: {error}")))?;
        let max_connections = usize::try_from(max_connections)
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or_else(|| {
                php_error("max_connections must be a positive platform-sized integer")
            })?;
        let event_capacity =
            NonZeroUsize::new(EVENT_QUEUE_CAPACITY).expect("fixed nonzero event capacity");
        let command_capacity =
            NonZeroUsize::new(SESSION_COMMAND_CAPACITY).expect("fixed nonzero command capacity");

        let mut state = session_runtime();
        if state.is_some() {
            return Err(php_error("Cobblestone session runtime is already started"));
        }

        let advertisement = format!(
            "MCPE;{server_name};{};;0;{}",
            cobblestone_codec::PROTOCOL_VERSION,
            max_connections.get()
        );
        let network = NetworkConfig::protocol8(bind_addr, max_connections, advertisement);
        let host = SessionHost::start(SessionHostConfig::new(
            network,
            codec_limits(),
            event_capacity,
            command_capacity,
        ))
        .map_err(|error| php_error(error.to_string()))?;

        *state = Some(PhpSessionRuntime { owner, host });
        Ok(())
    })
}

/// Returns whether the current PHP runtime owns the active session host.
#[php_function]
pub fn cobblestone_session_running() -> PhpResult<bool> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let state = session_runtime();
        match state.as_ref() {
            None => Ok(false),
            Some(runtime) if runtime.owner == owner => Ok(true),
            Some(_) => Err(php_error(
                "Cobblestone session runtime belongs to another PHP runtime",
            )),
        }
    })
}

/// Polls one bounded native session event on the owning PHP runtime.
///
/// Arrays are an internal bridge format:
/// connected => ["connected", sessionId, peer]
/// packet => ["packet", sessionId, packetId, binaryBody]
/// disconnected => ["disconnected", sessionId, reason]
#[php_function]
pub fn cobblestone_session_poll_event() -> PhpResult<Option<Vec<Zval>>> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let event = with_runtime(owner, SessionHost::try_recv_event)?;
        event.map(event_values).transpose()
    })
}

/// Queues one protocol-84 packet for a live session without blocking on network I/O.
#[php_function]
pub fn cobblestone_session_send(
    session_id: i64,
    packet_id: i64,
    body: Binary<u8>,
    delivery_mode: i64,
) -> PhpResult<()> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let session_id = owner_session_id(session_id)?;
        let packet_id =
            u8::try_from(packet_id).map_err(|_| php_error("packet id must fit one byte"))?;
        let delivery = delivery(delivery_mode)?;
        let packet = SessionPacket::new(packet_id, NativeBuffer::from_vec(body.into()));
        with_runtime(owner, |host| host.try_send(session_id, packet, delivery))
    })
}

fn bootstrap_session_packet(packet: BootstrapPacket) -> PhpResult<SessionPacket> {
    let limits = codec_limits();
    let frame = encode_bootstrap_frame(&packet, limits)
        .map_err(|error| php_error(error.to_string()))?;
    let raw = decode_game_frame(frame.as_slice(), limits)
        .map_err(|error| php_error(error.to_string()))?;
    Ok(SessionPacket::new(raw.id(), raw.body().clone()))
}

fn validate_login_body(body: Vec<u8>) -> PhpResult<()> {
    let limits = codec_limits();
    let raw = RawPacket::new(packet_id::LOGIN, NativeBuffer::from_vec(body));
    let frame = encode_game_frame(&raw, limits)
        .map_err(|error| php_error(error.to_string()))?;

    match decode_bootstrap_frame(frame.as_slice(), limits)
        .map_err(|error| php_error(error.to_string()))?
    {
        BootstrapPacket::Login(_) => Ok(()),
        _ => Err(php_error("expected protocol-84 Login packet")),
    }
}

fn initial_bootstrap_packets() -> PhpResult<Vec<SessionPacket>> {
    [
        BootstrapPacket::PlayStatus(PlayStatusPacket::new(PlayStatusPacket::LOGIN_SUCCESS)),
        BootstrapPacket::StartGame(StartGamePacket {
            seed: -1,
            dimension: 0,
            generator: 1,
            gamemode: 0,
            entity_id: 0,
            spawn: [0, 64, 0],
            position: [0.5, 65.0, 0.5],
            level_id: "Cobblestone".to_owned(),
        }),
        BootstrapPacket::SetTime(SetTimePacket::new(0, true)),
        BootstrapPacket::SetSpawnPosition(SetSpawnPositionPacket::new(0, 64, 0)),
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

fn requested_chunk_radius(body: &[u8]) -> PhpResult<i32> {
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

fn empty_layered_chunk_payload() -> NativeBuffer {
    const BLOCK_IDS: usize = 16 * 16 * 128;
    const NIBBLE_ARRAY: usize = BLOCK_IDS / 2;
    const HEIGHT_MAP: usize = 16 * 16;
    const BIOME_COLORS: usize = 16 * 16 * 4;
    const EXTRA_DATA_COUNT: usize = 4;
    const TOTAL: usize =
        BLOCK_IDS + NIBBLE_ARRAY * 3 + HEIGHT_MAP + BIOME_COLORS + EXTRA_DATA_COUNT;

    let mut payload = Vec::with_capacity(TOTAL);
    payload.resize(BLOCK_IDS, 0);
    payload.resize(payload.len() + NIBBLE_ARRAY, 0);
    payload.resize(payload.len() + NIBBLE_ARRAY, 0xff);
    payload.resize(payload.len() + NIBBLE_ARRAY, 0);
    payload.resize(payload.len() + HEIGHT_MAP, 0);
    payload.resize(payload.len() + BIOME_COLORS, 0);
    payload.extend_from_slice(&0_u32.to_le_bytes());

    NativeBuffer::from_vec(payload)
}

fn full_chunk_packet(chunk_x: i32, chunk_z: i32, payload: &NativeBuffer) -> RawPacket {
    let mut body = Vec::with_capacity(13 + payload.len());
    body.extend_from_slice(&chunk_x.to_be_bytes());
    body.extend_from_slice(&chunk_z.to_be_bytes());
    body.push(CHUNK_ORDER_LAYERED);
    body.extend_from_slice(
        &u32::try_from(payload.len())
            .expect("fixed probe chunk payload fits protocol-84 length")
            .to_be_bytes(),
    );
    body.extend_from_slice(payload.as_slice());
    RawPacket::new(FULL_CHUNK_DATA_ID, NativeBuffer::from_vec(body))
}

fn spawn_probe_packets() -> PhpResult<Vec<SessionPacket>> {
    let mut packets = Vec::with_capacity(3);
    packets.push(SessionPacket::new(
        CHUNK_RADIUS_UPDATED_ID,
        NativeBuffer::copy_from_slice(&PROBE_CHUNK_RADIUS.to_be_bytes()),
    ));

    let payload = empty_layered_chunk_payload();
    let mut chunks = Vec::new();
    for chunk_x in -PROBE_CHUNK_RADIUS..=PROBE_CHUNK_RADIUS {
        for chunk_z in -PROBE_CHUNK_RADIUS..=PROBE_CHUNK_RADIUS {
            chunks.push(full_chunk_packet(chunk_x, chunk_z, &payload));
        }
    }
    packets.push(bootstrap_session_packet(BootstrapPacket::Batch(
        BatchPacket::new(chunks),
    ))?);
    packets.push(bootstrap_session_packet(BootstrapPacket::PlayStatus(
        PlayStatusPacket::new(PlayStatusPacket::PLAYER_SPAWN),
    ))?);

    Ok(packets)
}

fn queue_reliable_ordered(
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

/// Validates the fixed-target Login body and queues the initial protocol-84 bootstrap sequence.
///
/// This is kernel-internal compatibility machinery; gameplay/plugin APIs never call it directly.
#[php_function]
pub fn cobblestone_session_protocol84_accept_login(
    session_id: i64,
    body: Binary<u8>,
) -> PhpResult<()> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let session_id = owner_session_id(session_id)?;
        validate_login_body(body.into())?;
        queue_reliable_ordered(owner, session_id, initial_bootstrap_packets()?)
    })
}

/// Decodes RequestChunkRadius and queues the bounded synthetic spawn probe.
///
/// Returns the client-requested radius for owner-runtime observability. The compatibility probe
/// remains capped at radius two until the real world/chunk system replaces it.
#[php_function]
pub fn cobblestone_session_protocol84_spawn_probe(
    session_id: i64,
    body: Binary<u8>,
) -> PhpResult<i64> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let session_id = owner_session_id(session_id)?;
        let body: Vec<u8> = body.into();
        let requested = requested_chunk_radius(&body)?;
        queue_reliable_ordered(owner, session_id, spawn_probe_packets()?)?;
        Ok(i64::from(requested))
    })
}

/// Queues a disconnect for a live session.
#[php_function]
pub fn cobblestone_session_disconnect(session_id: i64) -> PhpResult<()> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let session_id = owner_session_id(session_id)?;
        with_runtime(owner, |host| host.try_disconnect(session_id))
    })
}

/// Stops the current PHP runtime's native session mechanism and joins its host thread.
#[php_function]
pub fn cobblestone_session_stop() -> PhpResult<()> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let runtime = {
            let mut state = session_runtime();
            let runtime = state
                .as_ref()
                .ok_or_else(|| php_error("Cobblestone session runtime is not started"))?;
            if runtime.owner != owner {
                return Err(php_error(
                    "Cobblestone session runtime belongs to another PHP runtime",
                ));
            }
            state
                .take()
                .ok_or_else(|| php_error("Cobblestone session runtime disappeared"))?
        };

        runtime
            .host
            .shutdown()
            .map_err(|error| php_error(error.to_string()))
    })
}

pub(crate) fn register_session_functions(module: ModuleBuilder) -> ModuleBuilder {
    module
        .function(wrap_function!(cobblestone_session_start))
        .function(wrap_function!(cobblestone_session_running))
        .function(wrap_function!(cobblestone_session_poll_event))
        .function(wrap_function!(cobblestone_session_send))
        .function(wrap_function!(cobblestone_session_protocol84_accept_login))
        .function(wrap_function!(cobblestone_session_protocol84_spawn_probe))
        .function(wrap_function!(cobblestone_session_disconnect))
        .function(wrap_function!(cobblestone_session_stop))
}

pub(crate) fn shutdown_session_runtime() {
    let runtime = session_runtime().take();
    if let Some(runtime) = runtime {
        let _ = runtime.host.shutdown();
    }
}
