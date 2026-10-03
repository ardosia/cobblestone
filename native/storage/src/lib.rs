mod async_io;
mod async_load;
mod async_save;
mod metadata;
mod record;
mod region;

use std::io;

use thiserror::Error;

pub use async_load::{
    AsyncLoadBuildError, AsyncLoadConfig, AsyncLoadService, LoadCompletion, LoadFailure,
    LoadRequestState, LoadSubmitError, LoadWorkerError, MAX_ASYNC_LOAD_COMPLETION_CAPACITY,
    MAX_ASYNC_LOAD_QUEUE_CAPACITY, MAX_ASYNC_LOAD_WORKERS,
};
pub use async_save::{
    AsyncSaveBuildError, AsyncSaveConfig, AsyncSaveService, CompactionCompletion,
    CompactionFailure, CompactionReceipt, CompactionSubmitError,
    MAX_ASYNC_SAVE_COMPLETION_CAPACITY, MAX_ASYNC_SAVE_QUEUE_CAPACITY, MAX_ASYNC_SAVE_WORKERS,
    SaveCompletion, SaveFailure, SaveReceipt, SaveSubmitError, SaveWorkerError,
};
pub use metadata::{
    MAX_GENERATOR_SETTINGS_BYTES, MAX_WORLD_METADATA_PAYLOAD_BYTES, MAX_WORLD_NAME_BYTES,
    TARGET_GAME_PROTOCOL, TARGET_RAKNET_PROTOCOL, TARGET_VERSION_MAJOR, TARGET_VERSION_MINOR,
    TARGET_VERSION_PATCH, WORLD_METADATA_ENVELOPE_BYTES, WORLD_METADATA_FILENAME,
    WORLD_METADATA_PAYLOAD_VERSION, WorldDirectory, WorldMetadata, decode_world_metadata,
    encode_world_metadata,
};
pub use record::{
    ADAPTIVE_COMPRESSION_MIN_SAVINGS, Compression, CompressionPolicy, ExtensionSection,
    StoredChunk, decode_chunk_record, encode_chunk_record, encode_chunk_record_with_policy,
};
pub use region::{
    INDEX_PAGE_BYTES, RECORD_AREA_OFFSET, REGION_HEADER_BYTES, RegionCompactionResult, RegionCoord,
    RegionFile, RegionSaveResult, RegionStats, STORAGE_REGION_EDGE,
};

pub const STORAGE_FORMAT_VERSION: u16 = 1;
pub const CHUNK_RECORD_VERSION: u16 = 1;
pub const LEGACY_SEMANTIC_PAYLOAD_VERSION: u16 = 1;
pub const SEMANTIC_PAYLOAD_VERSION: u16 = 2;
pub const MAX_CHUNK_PAYLOAD_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_CHUNK_RECORD_BYTES: usize = MAX_CHUNK_PAYLOAD_BYTES + 64 * 1024;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("storage I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("invalid region header: {0}")]
    InvalidRegionHeader(&'static str),
    #[error("region or world metadata belongs to a different world")]
    WorldMismatch,
    #[error("world metadata already exists")]
    WorldMetadataAlreadyExists,
    #[error("world metadata is invalid: {0}")]
    InvalidWorldMetadata(&'static str),
    #[error("world metadata size {size} exceeds limit {limit}")]
    WorldMetadataTooLarge { size: usize, limit: usize },
    #[error("world metadata generation space exhausted")]
    MetadataGenerationExhausted,
    #[error(
        "region coordinate mismatch: expected {expected_x}:{expected_z}, got {actual_x}:{actual_z}"
    )]
    RegionCoordinateMismatch {
        expected_x: i32,
        expected_z: i32,
        actual_x: i32,
        actual_z: i32,
    },
    #[error("both region index pages are invalid")]
    InvalidIndexPages,
    #[error("region index generation space exhausted")]
    IndexGenerationExhausted,
    #[error("chunk {x}:{z} does not belong to storage region {region_x}:{region_z}")]
    ChunkOutsideRegion {
        x: i32,
        z: i32,
        region_x: i32,
        region_z: i32,
    },
    #[error("duplicate chunk {x}:{z} in one storage commit")]
    DuplicateChunkInCommit { x: i32, z: i32 },
    #[error("chunk record is corrupt: {0}")]
    CorruptChunkRecord(&'static str),
    #[error("chunk payload is corrupt: {0}")]
    CorruptChunkPayload(&'static str),
    #[error("unsupported chunk compression method {0}")]
    UnsupportedCompression(u8),
    #[error("chunk payload size {size} exceeds limit {limit}")]
    PayloadTooLarge { size: usize, limit: usize },
    #[error("chunk record size {size} exceeds limit {limit}")]
    RecordTooLarge { size: usize, limit: usize },
    #[error("invalid native chunk snapshot: {0}")]
    InvalidSnapshot(&'static str),
    #[error("chunk record revisions do not match its index entry")]
    IndexRecordRevisionMismatch,
    #[error("chunk record coordinate does not match its index slot")]
    IndexRecordCoordinateMismatch,
}
