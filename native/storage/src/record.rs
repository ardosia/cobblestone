use cobblestone_world::{ChunkCoord, ChunkImport};

use crate::StorageError;

mod codec;
mod payload;

pub use codec::{decode_chunk_record, encode_chunk_record, encode_chunk_record_with_policy};

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum Compression {
    None = 0,
    #[default]
    Zstd = 1,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub enum CompressionPolicy {
    None,
    Zstd,
    #[default]
    Adaptive,
}

pub const ADAPTIVE_COMPRESSION_MIN_SAVINGS: usize = 4096;

impl Compression {
    pub(super) fn from_byte(value: u8) -> Result<Self, StorageError> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Zstd),
            other => Err(StorageError::UnsupportedCompression(other)),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ExtensionSection {
    pub tag: u16,
    pub version: u16,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct StoredChunk {
    pub position: ChunkCoord,
    pub compression: Compression,
    pub import: ChunkImport,
    pub extensions: Vec<ExtensionSection>,
}
