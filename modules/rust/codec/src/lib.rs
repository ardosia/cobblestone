#![forbid(unsafe_code)]

//! Fixed-target MCPE 0.15.10 / protocol-84 binary wire mechanisms.
//!
//! This crate owns packet framing and binary representation only. RakNet transport and gameplay
//! semantics remain separate layers.

mod batch;
mod binary;
mod chunk;
mod error;
mod frame;
mod limits;
mod nbt;
mod packet;

pub use batch::BatchPacket;
pub use chunk::{
    CHUNK_BLOCK_COUNT, CHUNK_COLUMN_COUNT, CHUNK_NIBBLE_BYTES, CHUNK_ORDER_LAYERED,
    FULL_CHUNK_DATA_ID, Protocol84ChunkSnapshot, encode_protocol84_full_chunk_data,
};
pub use error::{CodecError, LimitKind};
pub use frame::{GAME_MARKER, RawPacket, decode_game_frame, encode_game_frame};
pub use limits::{CodecLimits, LOGIN_MAX_DECOMPRESSED_BYTES};
pub use nbt::{NamedNbt, NbtDocument, NbtLimits, NbtTag, NbtValue};
pub use packet::{
    AdventureFlags, AdventureSettingsPacket, BootstrapPacket, DisconnectPacket, LoginPacket,
    PROTOCOL_VERSION, PlayStatusPacket, SetDifficultyPacket, SetSpawnPositionPacket, SetTimePacket,
    StartGamePacket, decode_bootstrap_frame, encode_bootstrap_frame, packet_id,
};
