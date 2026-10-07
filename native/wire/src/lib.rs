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
mod movement;
mod nbt;
mod packet;
mod update;

pub use batch::BatchPacket;
pub use chunk::{
    CHUNK_ORDER_LAYERED, ChunkWireView, FULL_CHUNK_DATA_ID, encode_chunk_unload, encode_full_chunk,
};
pub use error::{CodecError, LimitKind};
pub use frame::{GAME_MARKER, RawPacket, decode_game_frame, encode_game_frame};
pub use limits::{CodecLimits, LOGIN_MAX_DECOMPRESSED_BYTES};
pub use movement::{MovePlayerMode, MovePlayerPacket, decode_move_player};
pub use nbt::{NamedNbt, NbtDocument, NbtLimits, NbtTag, NbtValue};
pub use packet::{
    AdventureFlags, AdventureSettingsPacket, BootstrapPacket, DimensionId, DisconnectPacket,
    LoginPacket, PlayStatusPacket, SetDifficultyPacket, SetSpawnPositionPacket, SetTimePacket,
    StartGamePacket, decode_bootstrap_frame, decode_bootstrap_packet, encode_bootstrap_frame,
    encode_bootstrap_packet, packet_id,
};
pub use update::{
    UPDATE_BLOCK_FLAG_ALL_PRIORITY, UPDATE_BLOCK_FLAG_NEIGHBORS, UPDATE_BLOCK_FLAG_NETWORK,
    UPDATE_BLOCK_FLAG_NOGRAPHIC, UPDATE_BLOCK_FLAG_PRIORITY, encode_update_block,
};
