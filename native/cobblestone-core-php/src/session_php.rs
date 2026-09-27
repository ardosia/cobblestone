use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::sync::{Mutex, MutexGuard};

use cobblestone_codec::CodecLimits;
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
        .function(wrap_function!(cobblestone_session_disconnect))
        .function(wrap_function!(cobblestone_session_stop))
}

pub(crate) fn shutdown_session_runtime() {
    let runtime = session_runtime().take();
    if let Some(runtime) = runtime {
        let _ = runtime.host.shutdown();
    }
}
