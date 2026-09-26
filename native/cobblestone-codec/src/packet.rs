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
    /// World spawn position.
    pub const SET_SPAWN_POSITION: u8 = 0x26;
    /// Adventure/player permission flags.
    pub const ADVENTURE_SETTINGS: u8 = 0x31;
    /// World difficulty.
    pub const SET_DIFFICULTY: u8 = 0x35;
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

/// Server world-time update.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct SetTimePacket {
    time: i32,
    started: bool,
}

impl SetTimePacket {
    /// Creates a world-time update.
    pub const fn new(time: i32, started: bool) -> Self {
        Self { time, started }
    }

    /// Current world time.
    pub const fn time(self) -> i32 {
        self.time
    }

    /// Whether the day/night cycle is running.
    pub const fn started(self) -> bool {
        self.started
    }
}

/// Initial protocol-84 world/session state.
#[derive(Debug, Clone, PartialEq)]
pub struct StartGamePacket {
    /// World seed.
    pub seed: i32,
    /// Dimension byte.
    pub dimension: u8,
    /// Generator identifier.
    pub generator: i32,
    /// Game-mode identifier.
    pub gamemode: i32,
    /// Player entity identifier.
    pub entity_id: i64,
    /// Integer spawn coordinates.
    pub spawn: [i32; 3],
    /// Floating player coordinates.
    pub position: [f32; 3],
    /// Historical trailing level/session string.
    pub level_id: String,
}

/// Server world-spawn update.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct SetSpawnPositionPacket {
    position: [i32; 3],
}

impl SetSpawnPositionPacket {
    /// Creates a spawn-position update.
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self {
            position: [x, y, z],
        }
    }

    /// Integer spawn coordinates.
    pub const fn position(self) -> [i32; 3] {
        self.position
    }
}

/// Server difficulty update.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct SetDifficultyPacket {
    difficulty: i32,
}

impl SetDifficultyPacket {
    /// Creates a difficulty update.
    pub const fn new(difficulty: i32) -> Self {
        Self { difficulty }
    }

    /// Difficulty identifier.
    pub const fn difficulty(self) -> i32 {
        self.difficulty
    }
}

/// Protocol-84 adventure and permission flags.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct AdventureSettingsPacket {
    flags: i32,
    user_permission: i32,
    global_permission: i32,
}

impl AdventureSettingsPacket {
    /// Creates an adventure-settings update.
    pub const fn new(flags: i32, user_permission: i32, global_permission: i32) -> Self {
        Self {
            flags,
            user_permission,
            global_permission,
        }
    }

    /// Adventure/player capability flags.
    pub const fn flags(self) -> i32 {
        self.flags
    }

    /// User permission level.
    pub const fn user_permission(self) -> i32 {
        self.user_permission
    }

    /// Global permission level.
    pub const fn global_permission(self) -> i32 {
        self.global_permission
    }
}

/// Typed bootstrap subset implemented by the initial C006 slice.
#[derive(Debug, Clone, PartialEq)]
pub enum BootstrapPacket {
    /// Login envelope.
    Login(LoginPacket),
    /// Play/login status.
    PlayStatus(PlayStatusPacket),
    /// Disconnect message.
    Disconnect(DisconnectPacket),
    /// World-time update.
    SetTime(SetTimePacket),
    /// Initial world/session state.
    StartGame(StartGamePacket),
    /// World spawn position.
    SetSpawnPosition(SetSpawnPositionPacket),
    /// Adventure/player permission flags.
    AdventureSettings(AdventureSettingsPacket),
    /// World difficulty.
    SetDifficulty(SetDifficultyPacket),
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
        BootstrapPacket::SetTime(packet) => {
            let mut writer = Writer::new();
            writer.put_i32_be(packet.time());
            writer.put_u8(u8::from(packet.started()));
            RawPacket::new(
                packet_id::SET_TIME,
                NativeBuffer::from_vec(writer.into_vec()),
            )
        }
        BootstrapPacket::StartGame(packet) => {
            let mut writer = Writer::new();
            writer.put_i32_be(packet.seed);
            writer.put_u8(packet.dimension);
            writer.put_i32_be(packet.generator);
            writer.put_i32_be(packet.gamemode);
            writer.put_i64_be(packet.entity_id);
            for coordinate in packet.spawn {
                writer.put_i32_be(coordinate);
            }
            for coordinate in packet.position {
                writer.put_f32_be(coordinate);
            }
            writer.put_u8(1);
            writer.put_u8(1);
            writer.put_u8(0);
            writer.put_string_u16("start-game level id", &packet.level_id)?;
            RawPacket::new(
                packet_id::START_GAME,
                NativeBuffer::from_vec(writer.into_vec()),
            )
        }
        BootstrapPacket::SetSpawnPosition(packet) => {
            let mut writer = Writer::new();
            for coordinate in packet.position() {
                writer.put_i32_be(coordinate);
            }
            RawPacket::new(
                packet_id::SET_SPAWN_POSITION,
                NativeBuffer::from_vec(writer.into_vec()),
            )
        }
        BootstrapPacket::AdventureSettings(packet) => {
            let mut writer = Writer::new();
            writer.put_i32_be(packet.flags());
            writer.put_i32_be(packet.user_permission());
            writer.put_i32_be(packet.global_permission());
            RawPacket::new(
                packet_id::ADVENTURE_SETTINGS,
                NativeBuffer::from_vec(writer.into_vec()),
            )
        }
        BootstrapPacket::SetDifficulty(packet) => {
            let mut writer = Writer::new();
            writer.put_i32_be(packet.difficulty());
            RawPacket::new(
                packet_id::SET_DIFFICULTY,
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
        packet_id::SET_TIME => {
            let mut reader = Reader::new(raw.body().as_slice());
            let time = reader.read_i32_be()?;
            let started = reader.read_u8()? != 0;
            reader.finish()?;
            Ok(BootstrapPacket::SetTime(SetTimePacket::new(time, started)))
        }
        packet_id::START_GAME => decode_start_game(raw.body().as_slice()),
        packet_id::SET_SPAWN_POSITION => {
            let mut reader = Reader::new(raw.body().as_slice());
            let packet = SetSpawnPositionPacket::new(
                reader.read_i32_be()?,
                reader.read_i32_be()?,
                reader.read_i32_be()?,
            );
            reader.finish()?;
            Ok(BootstrapPacket::SetSpawnPosition(packet))
        }
        packet_id::ADVENTURE_SETTINGS => {
            let mut reader = Reader::new(raw.body().as_slice());
            let packet = AdventureSettingsPacket::new(
                reader.read_i32_be()?,
                reader.read_i32_be()?,
                reader.read_i32_be()?,
            );
            reader.finish()?;
            Ok(BootstrapPacket::AdventureSettings(packet))
        }
        packet_id::SET_DIFFICULTY => {
            let mut reader = Reader::new(raw.body().as_slice());
            let difficulty = reader.read_i32_be()?;
            reader.finish()?;
            Ok(BootstrapPacket::SetDifficulty(SetDifficultyPacket::new(
                difficulty,
            )))
        }
        packet_id::BATCH => {
            BatchPacket::decode_body(raw.body().as_slice(), limits).map(BootstrapPacket::Batch)
        }
        id => Err(CodecError::UnsupportedPacket { id }),
    }
}

fn decode_start_game(body: &[u8]) -> Result<BootstrapPacket, CodecError> {
    let mut reader = Reader::new(body);
    let seed = reader.read_i32_be()?;
    let dimension = reader.read_u8()?;
    let generator = reader.read_i32_be()?;
    let gamemode = reader.read_i32_be()?;
    let entity_id = reader.read_i64_be()?;
    let spawn = [
        reader.read_i32_be()?,
        reader.read_i32_be()?,
        reader.read_i32_be()?,
    ];
    let position = [
        reader.read_f32_be()?,
        reader.read_f32_be()?,
        reader.read_f32_be()?,
    ];
    require_fixed_byte(&mut reader, "start-game flag 1", 1)?;
    require_fixed_byte(&mut reader, "start-game flag 2", 1)?;
    require_fixed_byte(&mut reader, "start-game flag 3", 0)?;
    let level_id = reader.read_string_u16()?;
    reader.finish()?;

    Ok(BootstrapPacket::StartGame(StartGamePacket {
        seed,
        dimension,
        generator,
        gamemode,
        entity_id,
        spawn,
        position,
        level_id,
    }))
}

fn require_fixed_byte(
    reader: &mut Reader<'_>,
    field: &'static str,
    expected: u8,
) -> Result<(), CodecError> {
    let actual = reader.read_u8()?;
    if actual == expected {
        Ok(())
    } else {
        Err(CodecError::InvalidFixedByte {
            field,
            expected,
            actual,
        })
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
