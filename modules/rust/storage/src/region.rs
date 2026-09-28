use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use cobblestone_core::{ChunkCoord, ChunkSnapshot};

use crate::{
    CompressionPolicy, MAX_CHUNK_RECORD_BYTES, STORAGE_FORMAT_VERSION, StorageError, StoredChunk,
    decode_chunk_record, encode_chunk_record_with_policy,
};

pub const STORAGE_REGION_EDGE: i32 = 16;
pub const REGION_HEADER_BYTES: usize = 64;
pub const INDEX_PAGE_BYTES: usize = 16 * 1024;
const INDEX_PAGE_COUNT: usize = 2;
const INDEX_ENTRY_COUNT: usize = 256;
const INDEX_ENTRY_BYTES: usize = 40;
const INDEX_HEADER_BYTES: usize = 32;
pub const RECORD_AREA_OFFSET: u64 =
    (REGION_HEADER_BYTES + INDEX_PAGE_BYTES * INDEX_PAGE_COUNT) as u64;

const REGION_MAGIC: &[u8; 4] = b"CBRG";
const INDEX_MAGIC: &[u8; 4] = b"CBIX";

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

#[derive(Debug, Clone, Copy, Default)]
struct IndexEntry {
    record_offset: u64,
    record_len: u32,
    record_crc32c: u32,
    terrain_revision: u64,
    light_revision: u64,
}

#[derive(Debug, Clone)]
struct RegionIndex {
    generation: u64,
    entries: [Option<IndexEntry>; INDEX_ENTRY_COUNT],
}

impl RegionIndex {
    fn empty(generation: u64) -> Self {
        Self {
            generation,
            entries: [None; INDEX_ENTRY_COUNT],
        }
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

pub struct RegionFile {
    file: File,
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
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)?;

        let len = file.metadata()?.len();
        if len == 0 {
            initialize_region(&mut file, world_uuid, coord)?;
            sync_parent_directory(path)?;
        } else if len < RECORD_AREA_OFFSET {
            return Err(StorageError::InvalidRegionHeader(
                "file is shorter than fixed header/index area",
            ));
        }

        Self::from_open_file(file, world_uuid, coord)
    }

    pub fn open_existing(
        path: impl AsRef<Path>,
        world_uuid: [u8; 16],
        coord: RegionCoord,
    ) -> Result<Option<Self>, StorageError> {
        let mut options = OpenOptions::new();
        options.read(true).write(true);

        let file = match options.open(path.as_ref()) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        if file.metadata()?.len() < RECORD_AREA_OFFSET {
            return Err(StorageError::InvalidRegionHeader(
                "file is shorter than fixed header/index area",
            ));
        }

        Self::from_open_file(file, world_uuid, coord).map(Some)
    }

    fn from_open_file(
        mut file: File,
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
        if decoded.position != position {
            return Err(StorageError::IndexRecordCoordinateMismatch);
        }
        if decoded.import.terrain_revision != entry.terrain_revision
            || decoded.import.light_revision != entry.light_revision
        {
            return Err(StorageError::IndexRecordRevisionMismatch);
        }
        Ok(Some(decoded))
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

fn initialize_region(
    file: &mut File,
    world_uuid: [u8; 16],
    coord: RegionCoord,
) -> Result<(), StorageError> {
    let header = encode_region_header(world_uuid, coord);
    let page_a = encode_index_page(&RegionIndex::empty(1), coord);
    let page_b = encode_index_page(&RegionIndex::empty(0), coord);

    file.seek(SeekFrom::Start(0))?;
    file.write_all(&header)?;
    file.write_all(&page_a)?;
    file.write_all(&page_b)?;
    file.set_len(RECORD_AREA_OFFSET)?;
    file.sync_all()?;
    Ok(())
}

fn encode_region_header(world_uuid: [u8; 16], coord: RegionCoord) -> [u8; REGION_HEADER_BYTES] {
    let mut header = [0_u8; REGION_HEADER_BYTES];
    header[0..4].copy_from_slice(REGION_MAGIC);
    put_u16(&mut header, 4, STORAGE_FORMAT_VERSION);
    put_u16(&mut header, 6, REGION_HEADER_BYTES as u16);
    put_i32(&mut header, 8, coord.x);
    put_i32(&mut header, 12, coord.z);
    header[16..32].copy_from_slice(&world_uuid);
    put_u16(&mut header, 32, STORAGE_REGION_EDGE as u16);
    put_u16(&mut header, 34, INDEX_PAGE_BYTES as u16);
    header[36] = INDEX_PAGE_COUNT as u8;
    put_u64(&mut header, 40, RECORD_AREA_OFFSET);
    let header_crc = crc32c::crc32c(&header[..60]);
    put_u32(&mut header, 60, header_crc);
    header
}

fn validate_region_header(
    header: &[u8; REGION_HEADER_BYTES],
    world_uuid: [u8; 16],
    coord: RegionCoord,
) -> Result<(), StorageError> {
    if &header[0..4] != REGION_MAGIC {
        return Err(StorageError::InvalidRegionHeader("bad magic"));
    }
    if read_u16(header, 4) != STORAGE_FORMAT_VERSION {
        return Err(StorageError::InvalidRegionHeader(
            "unsupported format version",
        ));
    }
    if usize::from(read_u16(header, 6)) != REGION_HEADER_BYTES {
        return Err(StorageError::InvalidRegionHeader("bad header length"));
    }
    if crc32c::crc32c(&header[..60]) != read_u32(header, 60) {
        return Err(StorageError::InvalidRegionHeader("checksum mismatch"));
    }

    let actual = RegionCoord::new(read_i32(header, 8), read_i32(header, 12));
    if actual != coord {
        return Err(StorageError::RegionCoordinateMismatch {
            expected_x: coord.x,
            expected_z: coord.z,
            actual_x: actual.x,
            actual_z: actual.z,
        });
    }
    if header[16..32] != world_uuid {
        return Err(StorageError::WorldMismatch);
    }
    if i32::from(read_u16(header, 32)) != STORAGE_REGION_EDGE
        || usize::from(read_u16(header, 34)) != INDEX_PAGE_BYTES
        || usize::from(header[36]) != INDEX_PAGE_COUNT
        || read_u64(header, 40) != RECORD_AREA_OFFSET
    {
        return Err(StorageError::InvalidRegionHeader(
            "fixed layout constants mismatch",
        ));
    }
    Ok(())
}

fn encode_index_page(index: &RegionIndex, coord: RegionCoord) -> [u8; INDEX_PAGE_BYTES] {
    let mut page = [0_u8; INDEX_PAGE_BYTES];
    page[0..4].copy_from_slice(INDEX_MAGIC);
    put_u16(&mut page, 4, STORAGE_FORMAT_VERSION);
    put_u16(&mut page, 6, INDEX_PAGE_BYTES as u16);
    put_u64(&mut page, 8, index.generation);
    put_i32(&mut page, 16, coord.x);
    put_i32(&mut page, 20, coord.z);
    put_u32(&mut page, 24, INDEX_ENTRY_COUNT as u32);

    for (slot, entry) in index.entries.iter().enumerate() {
        let Some(entry) = entry else {
            continue;
        };
        let base = INDEX_HEADER_BYTES + slot * INDEX_ENTRY_BYTES;
        put_u64(&mut page, base, entry.record_offset);
        put_u32(&mut page, base + 8, entry.record_len);
        put_u32(&mut page, base + 12, entry.record_crc32c);
        put_u64(&mut page, base + 16, entry.terrain_revision);
        put_u64(&mut page, base + 24, entry.light_revision);
    }

    let crc_offset = INDEX_PAGE_BYTES - 4;
    let page_crc = crc32c::crc32c(&page[..crc_offset]);
    put_u32(&mut page, crc_offset, page_crc);
    page
}

fn decode_index_page(page: &[u8; INDEX_PAGE_BYTES], coord: RegionCoord) -> Option<RegionIndex> {
    let crc_offset = INDEX_PAGE_BYTES - 4;
    if &page[0..4] != INDEX_MAGIC
        || read_u16(page, 4) != STORAGE_FORMAT_VERSION
        || usize::from(read_u16(page, 6)) != INDEX_PAGE_BYTES
        || read_i32(page, 16) != coord.x
        || read_i32(page, 20) != coord.z
        || read_u32(page, 24) as usize != INDEX_ENTRY_COUNT
        || crc32c::crc32c(&page[..crc_offset]) != read_u32(page, crc_offset)
    {
        return None;
    }

    let mut entries = [None; INDEX_ENTRY_COUNT];
    for (slot, entry) in entries.iter_mut().enumerate() {
        let base = INDEX_HEADER_BYTES + slot * INDEX_ENTRY_BYTES;
        let record_offset = read_u64(page, base);
        if record_offset == 0 {
            continue;
        }
        let record_len = read_u32(page, base + 8);
        if record_offset < RECORD_AREA_OFFSET
            || record_len == 0
            || record_len as usize > MAX_CHUNK_RECORD_BYTES
        {
            return None;
        }
        *entry = Some(IndexEntry {
            record_offset,
            record_len,
            record_crc32c: read_u32(page, base + 12),
            terrain_revision: read_u64(page, base + 16),
            light_revision: read_u64(page, base + 24),
        });
    }

    Some(RegionIndex {
        generation: read_u64(page, 8),
        entries,
    })
}

fn index_page_offset(page: usize) -> u64 {
    (REGION_HEADER_BYTES + page * INDEX_PAGE_BYTES) as u64
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> Result<(), StorageError> {
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn sync_parent_directory(_path: &Path) -> Result<(), StorageError> {
    Ok(())
}

fn read_exact_at<const N: usize>(file: &mut File, offset: u64) -> Result<[u8; N], StorageError> {
    let mut bytes = [0_u8; N];
    file.seek(SeekFrom::Start(offset))?;
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn put_i32(bytes: &mut [u8], offset: usize, value: i32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ])
}

fn read_i32(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

#[cfg(test)]
mod tests {
    use super::{RegionCoord, STORAGE_REGION_EDGE};
    use cobblestone_core::ChunkCoord;

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
