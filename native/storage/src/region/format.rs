use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use crate::{MAX_CHUNK_RECORD_BYTES, STORAGE_FORMAT_VERSION, StorageError};

use super::{RegionCoord, STORAGE_REGION_EDGE};

pub const REGION_HEADER_BYTES: usize = 64;
pub const INDEX_PAGE_BYTES: usize = 16 * 1024;
const INDEX_PAGE_COUNT: usize = 2;
pub(super) const INDEX_ENTRY_COUNT: usize = 256;
const INDEX_ENTRY_BYTES: usize = 40;
const INDEX_HEADER_BYTES: usize = 32;
pub const RECORD_AREA_OFFSET: u64 =
    (REGION_HEADER_BYTES + INDEX_PAGE_BYTES * INDEX_PAGE_COUNT) as u64;

const REGION_MAGIC: &[u8; 4] = b"CBRG";
const INDEX_MAGIC: &[u8; 4] = b"CBIX";

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct IndexEntry {
    pub(super) record_offset: u64,
    pub(super) record_len: u32,
    pub(super) record_crc32c: u32,
    pub(super) terrain_revision: u64,
    pub(super) light_revision: u64,
}

#[derive(Debug, Clone)]
pub(super) struct RegionIndex {
    pub(super) generation: u64,
    pub(super) entries: [Option<IndexEntry>; INDEX_ENTRY_COUNT],
}

impl RegionIndex {
    pub(super) fn empty(generation: u64) -> Self {
        Self {
            generation,
            entries: [None; INDEX_ENTRY_COUNT],
        }
    }
}

pub(super) fn initialize_region(
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

pub(super) fn validate_region_header(
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

pub(super) fn encode_index_page(index: &RegionIndex, coord: RegionCoord) -> [u8; INDEX_PAGE_BYTES] {
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

pub(super) fn decode_index_page(
    page: &[u8; INDEX_PAGE_BYTES],
    coord: RegionCoord,
) -> Option<RegionIndex> {
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

pub(super) fn index_page_offset(page: usize) -> u64 {
    (REGION_HEADER_BYTES + page * INDEX_PAGE_BYTES) as u64
}

#[cfg(unix)]
pub(super) fn sync_parent_directory(path: &Path) -> Result<(), StorageError> {
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

#[cfg(not(unix))]
pub(super) fn sync_parent_directory(_path: &Path) -> Result<(), StorageError> {
    Ok(())
}

pub(super) fn read_exact_at<const N: usize>(
    file: &mut File,
    offset: u64,
) -> Result<[u8; N], StorageError> {
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
