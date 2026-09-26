use cobblestone_core::NativeBuffer;

use crate::batch::{BatchPacket, compress_zlib, decompress_zlib_limited};
use crate::binary::{Reader, Writer};
use crate::frame::{RawPacket, check_limit};
use crate::{
    CodecError, CodecLimits, LOGIN_MAX_DECOMPRESSED_BYTES, LimitKind, decode_game_frame,
    encode_game_frame,
};

/// Fixed MCPE game protocol targeted by Cobblestone.
pub const PROTOCOL_VERSION: i32 = 84;

/// Initial protocol-84 packet ID table.
pub mod packet_id {
    /// Client Login.
    pub const LOGIN: u8 = 0x01;
    /// Server PlayStatus.
    pub const PLAY_STATUS: u8 = 0x02;
    /// Server encryption handshake.
    pub const SERVER_TO_CLIENT_HANDSHAKE: u8 = 0x03;
    /// Client encryption handshake response.
    pub const CLIENT_TO_SERVER_HANDSHAKE: u8 = 0x04;
    /// Disconnect text.
    pub const DISCONNECT: u8 = 0x05;
    /// Zlib-compressed packet batch.
    pub const BATCH: u8 = 0x06;
    /// Text/chat packet.
    pub const TEXT: u8 = 0x07;
    /// World time packet.
    pub const SET_TIME: u8 = 0x08;
    /// Initial world/session state.
    pub const START_GAME: u8 = 0x09;
}

/// Decoded protocol-84 Login envelope.
///
/// JWT/authentication semantics deliberately remain outside this binary codec.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LoginPacket {
    protocol: i32,
    chain_data: NativeBuffer,
    skin_jwt: NativeBuffer,
}

impl LoginPacket {
    /// Creates a protocol-84 login envelope from already serialized auth blobs.
    pub fn protocol84(chain_data: NativeBuffer, skin_jwt: NativeBuffer) -> Self {
        Self {
            protocol: PROTOCOL_VERSION,
            chain_data,
            skin_jwt,
        }
    }

    /// Game protocol number.
    pub const fn protocol(&self) -> i32 {
        self.protocol
    }

    /// Raw chain JSON bytes from the decompressed login body.
    pub const fn chain_data(&self) -> &NativeBuffer {
        &self.chain_data
    }

    /// Raw skin JWT bytes from the decompressed login body.
    pub const fn skin_jwt(&self) -> &NativeBuffer {
        &self.skin_jwt
    }
}

/// Server login/spawn status packet.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct PlayStatusPacket {
    status: i32,
}

impl PlayStatusPacket {
    /// Login accepted.
    pub const LOGIN_SUCCESS: i32 = 0;
    /// Client protocol is too old/new for this server.
    pub const LOGIN_FAILED_CLIENT: i32 = 1;
    /// Server protocol mismatch/failure status.
    pub const LOGIN_FAILED_SERVER: i32 = 2;
    /// Player spawn completed.
    pub const PLAYER_SPAWN: i32 = 3;

    /// Creates a status packet.
    pub const fn new(status: i32) -> Self {
        Self { status }
    }

    /// Status integer.
    pub const fn status(self) -> i32 {
        self.status
    }
}

/// Server/client disconnect packet.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DisconnectPacket {
    message: String,
}

impl DisconnectPacket {
    /// Creates a disconnect packet.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Human-readable disconnect text.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Typed bootstrap subset implemented by the initial C006 slice.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum BootstrapPacket {
    /// Login envelope.
    Login(LoginPacket),
    /// Play/login status.
    PlayStatus(PlayStatusPacket),
    /// Disconnect message.
    Disconnect(DisconnectPacket),
    /// Zlib packet batch.
    Batch(BatchPacket),
}

/// Decodes one outer 0xfe game frame into the implemented bootstrap subset.
pub fn decode_bootstrap_frame(
    input: &[u8],
    limits: CodecLimits,
) -> Result<BootstrapPacket, CodecError> {
    let raw = decode_game_frame(input, limits)?;
    decode_bootstrap_packet(raw, limits)
}

/// Encodes one implemented bootstrap packet as an outer 0xfe game frame.
pub fn encode_bootstrap_frame(
    packet: &BootstrapPacket,
    limits: CodecLimits,
) -> Result<NativeBuffer, CodecError> {
    let raw = match packet {
        BootstrapPacket::Login(packet) => {
            RawPacket::new(packet_id::LOGIN, encode_login(packet, limits)?)
        }
        BootstrapPacket::PlayStatus(packet) => {
            let mut writer = Writer::new();
            writer.put_i32_be(packet.status());
            RawPacket::new(
                packet_id::PLAY_STATUS,
                NativeBuffer::from_vec(writer.into_vec()),
            )
        }
        BootstrapPacket::Disconnect(packet) => {
            let mut writer = Writer::new();
            writer.put_string_u16("disconnect message", packet.message())?;
            RawPacket::new(
                packet_id::DISCONNECT,
                NativeBuffer::from_vec(writer.into_vec()),
            )
        }
        BootstrapPacket::Batch(packet) => {
            RawPacket::new(packet_id::BATCH, packet.encode_body(limits)?)
        }
    };
    encode_game_frame(&raw, limits)
}

fn decode_bootstrap_packet(
    raw: RawPacket,
    limits: CodecLimits,
) -> Result<BootstrapPacket, CodecError> {
    match raw.id() {
        packet_id::LOGIN => decode_login(raw.body().as_slice(), limits).map(BootstrapPacket::Login),
        packet_id::PLAY_STATUS => {
            let mut reader = Reader::new(raw.body().as_slice());
            let status = reader.read_i32_be()?;
            reader.finish()?;
            Ok(BootstrapPacket::PlayStatus(PlayStatusPacket::new(status)))
        }
        packet_id::DISCONNECT => {
            let mut reader = Reader::new(raw.body().as_slice());
            let message = reader.read_string_u16()?;
            reader.finish()?;
            Ok(BootstrapPacket::Disconnect(DisconnectPacket::new(message)))
        }
        packet_id::BATCH => {
            BatchPacket::decode_body(raw.body().as_slice(), limits).map(BootstrapPacket::Batch)
        }
        id => Err(CodecError::UnsupportedPacket { id }),
    }
}

fn decode_login(body: &[u8], limits: CodecLimits) -> Result<LoginPacket, CodecError> {
    let mut reader = Reader::new(body);
    let protocol = reader.read_i32_be()?;
    if protocol != PROTOCOL_VERSION {
        return Err(CodecError::UnsupportedProtocol {
            expected: PROTOCOL_VERSION,
            actual: protocol,
        });
    }

    let compressed_len = nonnegative_len("login compressed length", reader.read_i32_be()?)?;
    check_limit(
        LimitKind::LoginCompressed,
        compressed_len,
        limits.max_login_compressed_bytes(),
    )?;
    let compressed = reader.read_exact(compressed_len)?;
    reader.finish()?;

    let decompressed = decompress_zlib_limited(
        compressed,
        LOGIN_MAX_DECOMPRESSED_BYTES,
        LimitKind::LoginDecompressed,
    )?;
    let mut inner = Reader::new(&decompressed);
    let chain_len = nonnegative_len("login chain length", inner.read_i32_le()?)?;
    let chain_data = NativeBuffer::copy_from_slice(inner.read_exact(chain_len)?);
    let skin_len = nonnegative_len("login skin JWT length", inner.read_i32_le()?)?;
    let skin_jwt = NativeBuffer::copy_from_slice(inner.read_exact(skin_len)?);
    inner.finish()?;

    Ok(LoginPacket {
        protocol,
        chain_data,
        skin_jwt,
    })
}

fn encode_login(packet: &LoginPacket, limits: CodecLimits) -> Result<NativeBuffer, CodecError> {
    if packet.protocol != PROTOCOL_VERSION {
        return Err(CodecError::UnsupportedProtocol {
            expected: PROTOCOL_VERSION,
            actual: packet.protocol,
        });
    }

    let chain_len =
        i32::try_from(packet.chain_data.len()).map_err(|_| CodecError::LengthOutOfRange {
            field: "login chain length",
            value: packet.chain_data.len(),
            max: i32::MAX as usize,
        })?;
    let skin_len =
        i32::try_from(packet.skin_jwt.len()).map_err(|_| CodecError::LengthOutOfRange {
            field: "login skin JWT length",
            value: packet.skin_jwt.len(),
            max: i32::MAX as usize,
        })?;

    let mut inner = Writer::new();
    inner.put_i32_le(chain_len);
    inner.put_bytes(packet.chain_data.as_slice());
    inner.put_i32_le(skin_len);
    inner.put_bytes(packet.skin_jwt.as_slice());
    let inner = inner.into_vec();
    check_limit(
        LimitKind::LoginDecompressed,
        inner.len(),
        LOGIN_MAX_DECOMPRESSED_BYTES,
    )?;

    let compressed = compress_zlib(&inner)?;
    check_limit(
        LimitKind::LoginCompressed,
        compressed.len(),
        limits.max_login_compressed_bytes(),
    )?;
    let compressed_len =
        i32::try_from(compressed.len()).map_err(|_| CodecError::LengthOutOfRange {
            field: "login compressed length",
            value: compressed.len(),
            max: i32::MAX as usize,
        })?;

    let mut writer = Writer::with_capacity(8 + compressed.len());
    writer.put_i32_be(PROTOCOL_VERSION);
    writer.put_i32_be(compressed_len);
    writer.put_bytes(&compressed);
    Ok(NativeBuffer::from_vec(writer.into_vec()))
}

fn nonnegative_len(field: &'static str, value: i32) -> Result<usize, CodecError> {
    usize::try_from(value).map_err(|_| CodecError::NegativeLength { field, value })
}
