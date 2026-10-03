use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use cobblestone_world::{ChunkCoord, ChunkSnapshot};

use crate::{
    CompressionPolicy, MAX_CHUNK_RECORD_BYTES, StorageError, StoredChunk, decode_chunk_record,
    encode_chunk_record_with_policy,
};

mod compact;
mod format;

use format::{
    INDEX_ENTRY_COUNT, IndexEntry, RegionIndex, decode_index_page, encode_index_page,
    index_page_offset, initialize_region, read_exact_at, sync_parent_directory,
    validate_region_header,
};
pub use format::{INDEX_PAGE_BYTES, RECORD_AREA_OFFSET, REGION_HEADER_BYTES};

pub const STORAGE_REGION_EDGE: i32 = 16;

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub struct RegionCoord {
    pub x: i32,
    pub z: i32,
}

impl RegionCoord {
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    pub fn for_chunk(position: ChunkCoord) -> Self {
        Self {
            x: position.x().div_euclid(STORAGE_REGION_EDGE),
            z: position.z().div_euclid(STORAGE_REGION_EDGE),
        }
    }

    fn local_index(self, position: ChunkCoord) -> Result<usize, StorageError> {
        let actual = Self::for_chunk(position);
        if actual != self {
            return Err(StorageError::ChunkOutsideRegion {
                x: position.x(),
                z: position.z(),
                region_x: self.x,
                region_z: self.z,
            });
        }
        let local_x = position.x().rem_euclid(STORAGE_REGION_EDGE) as usize;
        let local_z = position.z().rem_euclid(STORAGE_REGION_EDGE) as usize;
        Ok(local_z * STORAGE_REGION_EDGE as usize + local_x)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct RegionStats {
    pub file_bytes: u64,
    pub record_bytes: u64,
    pub live_bytes: u64,
    pub dead_bytes: u64,
    pub indexed_chunks: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct RegionSaveResult {
    pub generation: u64,
    pub records: usize,
    pub bytes_appended: u64,
    pub stats: RegionStats,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct RegionCompactionResult {
    pub generation: u64,
    pub records: usize,
    pub bytes_reclaimed: u64,
    pub before: RegionStats,
    pub after: RegionStats,
}

pub struct RegionFile {
    file: File,
    path: PathBuf,
    world_uuid: [u8; 16],
    coord: RegionCoord,
    active_page: usize,
    index: RegionIndex,
}

impl RegionFile {
    pub fn open_or_create(
        path: impl AsRef<Path>,
        world_uuid: [u8; 16],
        coord: RegionCoord,
    ) -> Result<Self, StorageError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)?;

        let len = file.metadata()?.len();
        if len == 0 {
            initialize_region(&mut file, world_uuid, coord)?;
            sync_parent_directory(&path)?;
        } else if len < RECORD_AREA_OFFSET {
            return Err(StorageError::InvalidRegionHeader(
                "file is shorter than fixed header/index area",
            ));
        }

        Self::from_open_file(file, path, world_uuid, coord)
    }

    pub fn open_existing(
        path: impl AsRef<Path>,
        world_uuid: [u8; 16],
        coord: RegionCoord,
    ) -> Result<Option<Self>, StorageError> {
        let path = path.as_ref().to_path_buf();
        let mut options = OpenOptions::new();
        options.read(true).write(true);

        let file = match options.open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        if file.metadata()?.len() < RECORD_AREA_OFFSET {
            return Err(StorageError::InvalidRegionHeader(
                "file is shorter than fixed header/index area",
            ));
        }

        Self::from_open_file(file, path, world_uuid, coord).map(Some)
    }

    fn from_open_file(
        mut file: File,
        path: PathBuf,
        world_uuid: [u8; 16],
        coord: RegionCoord,
    ) -> Result<Self, StorageError> {
        let header = read_exact_at::<REGION_HEADER_BYTES>(&mut file, 0)?;
        validate_region_header(&header, world_uuid, coord)?;

        let page_a = read_exact_at::<INDEX_PAGE_BYTES>(&mut file, REGION_HEADER_BYTES as u64)?;
        let page_b = read_exact_at::<INDEX_PAGE_BYTES>(
            &mut file,
            (REGION_HEADER_BYTES + INDEX_PAGE_BYTES) as u64,
        )?;
        let decoded_a = decode_index_page(&page_a, coord);
        let decoded_b = decode_index_page(&page_b, coord);

        let (active_page, index) = match (decoded_a, decoded_b) {
            (Some(a), Some(b)) if b.generation > a.generation => (1, b),
            (Some(a), Some(_)) => (0, a),
            (Some(a), None) => (0, a),
            (None, Some(b)) => (1, b),
            (None, None) => return Err(StorageError::InvalidIndexPages),
        };

        Ok(Self {
            file,
            path,
            world_uuid,
            coord,
            active_page,
            index,
        })
    }

    pub const fn coord(&self) -> RegionCoord {
        self.coord
    }

    pub const fn world_uuid(&self) -> [u8; 16] {
        self.world_uuid
    }

    pub const fn generation(&self) -> u64 {
        self.index.generation
    }

    pub fn indexed_chunks(&self) -> usize {
        self.index
            .entries
            .iter()
            .filter(|entry| entry.is_some())
            .count()
    }

    pub fn stats(&self) -> Result<RegionStats, StorageError> {
        let file_bytes = self.file.metadata()?.len();
        if file_bytes < RECORD_AREA_OFFSET {
            return Err(StorageError::InvalidRegionHeader(
                "record area starts before fixed offset",
            ));
        }

        let record_bytes = file_bytes - RECORD_AREA_OFFSET;
        let live_bytes = self
            .index
            .entries
            .iter()
            .flatten()
            .map(|entry| u64::from(entry.record_len))
            .sum::<u64>();
        if live_bytes > record_bytes {
            return Err(StorageError::CorruptChunkRecord(
                "indexed live bytes exceed record area",
            ));
        }

        Ok(RegionStats {
            file_bytes,
            record_bytes,
            live_bytes,
            dead_bytes: record_bytes - live_bytes,
            indexed_chunks: self.indexed_chunks(),
        })
    }

    pub fn load_chunk(
        &mut self,
        position: ChunkCoord,
    ) -> Result<Option<StoredChunk>, StorageError> {
        let slot = self.coord.local_index(position)?;
        let Some(entry) = self.index.entries[slot] else {
            return Ok(None);
        };
        let (_record, decoded) = self.read_indexed_record(slot, entry)?;
        if decoded.position != position {
            return Err(StorageError::IndexRecordCoordinateMismatch);
        }
        Ok(Some(decoded))
    }

    fn read_indexed_record(
        &mut self,
        slot: usize,
        entry: IndexEntry,
    ) -> Result<(Vec<u8>, StoredChunk), StorageError> {
        let record_len = entry.record_len as usize;
        if record_len > MAX_CHUNK_RECORD_BYTES {
            return Err(StorageError::RecordTooLarge {
                size: record_len,
                limit: MAX_CHUNK_RECORD_BYTES,
            });
        }
        if entry.record_offset < RECORD_AREA_OFFSET {
            return Err(StorageError::CorruptChunkRecord(
                "index points into fixed header/index area",
            ));
        }

        let end = entry
            .record_offset
            .checked_add(u64::from(entry.record_len))
            .ok_or(StorageError::CorruptChunkRecord("record offset overflow"))?;
        if end > self.file.metadata()?.len() {
            return Err(StorageError::CorruptChunkRecord(
                "index points beyond end of region file",
            ));
        }

        let mut record = vec![0_u8; record_len];
        self.file.seek(SeekFrom::Start(entry.record_offset))?;
        self.file.read_exact(&mut record)?;
        if crc32c::crc32c(&record) != entry.record_crc32c {
            return Err(StorageError::CorruptChunkRecord(
                "indexed record checksum mismatch",
            ));
        }

        let decoded = decode_chunk_record(&record)?;
        if self.coord.local_index(decoded.position)? != slot {
            return Err(StorageError::IndexRecordCoordinateMismatch);
        }
        if decoded.import.terrain_revision != entry.terrain_revision
            || decoded.import.light_revision != entry.light_revision
        {
            return Err(StorageError::IndexRecordRevisionMismatch);
        }

        Ok((record, decoded))
    }

    pub fn save_chunk(
        &mut self,
        snapshot: &ChunkSnapshot,
        compression: CompressionPolicy,
    ) -> Result<RegionSaveResult, StorageError> {
        self.save_chunks(std::slice::from_ref(snapshot), compression)
    }

    pub fn save_chunks(
        &mut self,
        snapshots: &[ChunkSnapshot],
        compression: CompressionPolicy,
    ) -> Result<RegionSaveResult, StorageError> {
        if snapshots.is_empty() {
            return Ok(RegionSaveResult {
                generation: self.index.generation,
                records: 0,
                bytes_appended: 0,
                stats: self.stats()?,
            });
        }

        let mut seen = [false; INDEX_ENTRY_COUNT];
        let mut encoded = Vec::with_capacity(snapshots.len());
        for snapshot in snapshots {
            let slot = self.coord.local_index(snapshot.position())?;
            if seen[slot] {
                return Err(StorageError::DuplicateChunkInCommit {
                    x: snapshot.position().x(),
                    z: snapshot.position().z(),
                });
            }
            seen[slot] = true;
            encoded.push((
                slot,
                snapshot.position(),
                snapshot.terrain_revision(),
                snapshot.light_revision(),
                encode_chunk_record_with_policy(snapshot, compression, &[])?,
            ));
        }

        let mut end = self.file.seek(SeekFrom::End(0))?;
        if end < RECORD_AREA_OFFSET {
            return Err(StorageError::InvalidRegionHeader(
                "record area starts before fixed offset",
            ));
        }

        let mut next_index = self.index.clone();
        let start = end;
        for (slot, _position, terrain_revision, light_revision, record) in encoded {
            let record_len =
                u32::try_from(record.len()).map_err(|_| StorageError::RecordTooLarge {
                    size: record.len(),
                    limit: u32::MAX as usize,
                })?;
            let entry = IndexEntry {
                record_offset: end,
                record_len,
                record_crc32c: crc32c::crc32c(&record),
                terrain_revision,
                light_revision,
            };
            self.file.write_all(&record)?;
            end = end
                .checked_add(u64::from(record_len))
                .ok_or(StorageError::RecordTooLarge {
                    size: usize::MAX,
                    limit: MAX_CHUNK_RECORD_BYTES,
                })?;
            next_index.entries[slot] = Some(entry);
        }

        self.file.sync_data()?;

        next_index.generation = self
            .index
            .generation
            .checked_add(1)
            .ok_or(StorageError::IndexGenerationExhausted)?;
        let inactive_page = 1 - self.active_page;
        let page = encode_index_page(&next_index, self.coord);
        self.file
            .seek(SeekFrom::Start(index_page_offset(inactive_page)))?;
        self.file.write_all(&page)?;
        self.file.sync_data()?;

        self.active_page = inactive_page;
        self.index = next_index;

        Ok(RegionSaveResult {
            generation: self.index.generation,
            records: snapshots.len(),
            bytes_appended: end - start,
            stats: self.stats()?,
        })
    }
    pub fn sync_all(&self) -> Result<(), StorageError> {
        self.file.sync_all()?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::{RegionCoord, STORAGE_REGION_EDGE};
    use cobblestone_world::ChunkCoord;

    #[test]
    fn negative_chunks_use_floor_region_coordinates() {
        assert_eq!(
            RegionCoord::for_chunk(ChunkCoord::new(-1, -1)),
            RegionCoord::new(-1, -1)
        );
        assert_eq!(
            RegionCoord::for_chunk(ChunkCoord::new(-STORAGE_REGION_EDGE, 15)),
            RegionCoord::new(-1, 0)
        );
        assert_eq!(
            RegionCoord::for_chunk(ChunkCoord::new(-STORAGE_REGION_EDGE - 1, 16)),
            RegionCoord::new(-2, 1)
        );
    }
}
