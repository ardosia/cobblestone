#![forbid(unsafe_code)]

//! Fixed-target MCPE 0.15.10 / protocol-84 binary wire mechanisms.
//!
//! This crate owns packet framing and binary representation only. RakNet transport and gameplay
//! semantics remain separate layers.

mod batch;
mod binary;
mod error;
mod frame;
mod limits;
mod nbt;
mod packet;

pub use batch::BatchPacket;
pub use error::{CodecError, LimitKind};
pub use frame::{GAME_MARKER, RawPacket, decode_game_frame, encode_game_frame};
pub use limits::{CodecLimits, LOGIN_MAX_DECOMPRESSED_BYTES};
pub use nbt::{NamedNbt, NbtDocument, NbtLimits, NbtTag, NbtValue};
pub use packet::{
    AdventureSettingsPacket, BootstrapPacket, DisconnectPacket, LoginPacket, PROTOCOL_VERSION,
    PlayStatusPacket, SetDifficultyPacket, SetSpawnPositionPacket, SetTimePacket, StartGamePacket,
    decode_bootstrap_frame, encode_bootstrap_frame, packet_id,
};
