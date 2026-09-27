//! Compatibility-only protocol-84 bootstrap/probe exports.
//!
//! Production PHP composition does not call these functions. They remain registered so the
//! previously published internal native ABI does not disappear during source cleanup.

use cobblestone_codec::{BatchPacket, BootstrapPacket, PlayStatusPacket, RawPacket};
use cobblestone_core::NativeBuffer;
use cobblestone_session::SessionPacket;
use ext_php_rs::binary::Binary;
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::{php_boundary, php_error};
use crate::runtime::current_runtime_id;
use crate::session::bridge::owner_session_id;

use super::join::{
    WorldBootstrap, bootstrap_session_packet, initial_bootstrap_packets, queue_reliable_ordered,
    requested_chunk_radius, validate_login_body,
};

const PROBE_CHUNK_RADIUS: i32 = 2;
const FULL_CHUNK_DATA_ID: u8 = 0x34;
const CHUNK_ORDER_LAYERED: u8 = 1;
const CHUNK_RADIUS_UPDATED_ID: u8 = 0x3e;

fn legacy_bootstrap() -> WorldBootstrap {
    WorldBootstrap {
        seed: -1,
        generator: 1,
        spawn: [0, 64, 0],
        position: [0.5, 65.0, 0.5],
        time: 0,
        time_started: true,
        level_id: "Cobblestone".to_owned(),
    }
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
            .expect("fixed compatibility probe payload fits protocol-84 length")
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

#[php_function]
#[php(name = "cobblestone_session_protocol84_accept_login")]
pub fn cobblestone_session_protocol84_accept_login(
    session_id: i64,
    body: Binary<u8>,
) -> PhpResult<()> {
    php_boundary(|| {
        let owner = current_runtime_id().map_err(php_error)?;
        let session_id = owner_session_id(session_id)?;
        validate_login_body(body.into())?;
        queue_reliable_ordered(
            owner,
            session_id,
            initial_bootstrap_packets(&legacy_bootstrap())?,
        )
    })
}

#[php_function]
#[php(name = "cobblestone_session_protocol84_spawn_probe")]
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

pub(crate) fn register(module: ModuleBuilder) -> ModuleBuilder {
    module
        .function(wrap_function!(cobblestone_session_protocol84_accept_login))
        .function(wrap_function!(cobblestone_session_protocol84_spawn_probe))
}
