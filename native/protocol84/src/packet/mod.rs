use bitflags::bitflags;
use cobblestone_runtime::NativeBuffer;

use crate::batch::BatchPacket;

mod codec;

pub use codec::{
    decode_bootstrap_frame, decode_bootstrap_packet, encode_bootstrap_frame,
    encode_bootstrap_packet,
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
    /// Client/server player movement.
    pub const MOVE_PLAYER: u8 = 0x10;
    /// Authoritative block-state update.
    pub const UPDATE_BLOCK: u8 = 0x13;
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

bitflags! {
    /// Protocol-84 AdventureSettings flag bits.
    ///
    /// The matching 0.15.10 source exposes the full survival and creative bit patterns. Unknown
    /// bits are retained when decoding so fixed-target packets are never silently normalized.
    #[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
    pub struct AdventureFlags: u32 {
        /// Full survival flag set observed for MCPE 0.15.10.
        const SURVIVAL = 0x4e;
        /// Full creative flag set observed for MCPE 0.15.10.
        const CREATIVE = 0xce;
    }
}

/// Protocol-84 adventure and permission settings.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct AdventureSettingsPacket {
    flags: AdventureFlags,
    user_permission: i32,
    global_permission: i32,
}

impl AdventureSettingsPacket {
    /// Creates an adventure-settings update.
    pub const fn new(flags: i32, user_permission: i32, global_permission: i32) -> Self {
        Self {
            flags: AdventureFlags::from_bits_retain(flags as u32),
            user_permission,
            global_permission,
        }
    }

    /// Raw adventure/player capability bits as carried on the wire.
    pub const fn flags(self) -> i32 {
        self.flags.bits() as i32
    }

    /// Typed adventure/player capability bits, retaining unknown fixed-target flags.
    pub const fn adventure_flags(self) -> AdventureFlags {
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
