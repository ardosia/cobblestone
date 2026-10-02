use cobblestone_protocol84::{
    CHUNK_BLOCK_COUNT, CHUNK_COLUMN_COUNT, CHUNK_NIBBLE_BYTES, Protocol84ChunkSnapshot, RawPacket,
    encode_protocol84_full_chunk_data,
};
use ext_php_rs::exception::PhpResult;

use crate::boundary::php_error;

const MAX_INITIAL_CHUNKS: usize = 49;
const MAX_PROJECTION_BYTES: usize = 4 * 1024 * 1024;

struct ProjectionReader<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> ProjectionReader<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }

    fn read_exact(&mut self, len: usize) -> PhpResult<&'a [u8]> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| php_error("chunk projection offset overflow"))?;
        if end > self.input.len() {
            return Err(php_error(format!(
                "truncated chunk projection: needed {len} bytes with {} remaining",
                self.input.len().saturating_sub(self.offset)
            )));
        }
        let bytes = &self.input[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }

    fn read_u16_le(&mut self) -> PhpResult<u16> {
        let bytes = self.read_exact(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32_le(&mut self) -> PhpResult<u32> {
        let bytes = self.read_exact(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_i32_le(&mut self) -> PhpResult<i32> {
        let bytes = self.read_exact(4)?;
        Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn finish(self) -> PhpResult<()> {
        if self.offset == self.input.len() {
            Ok(())
        } else {
            Err(php_error(format!(
                "chunk projection has {} trailing bytes",
                self.input.len() - self.offset
            )))
        }
    }
}

pub(super) fn decode_initial_chunk_projection(
    input: &[u8],
    expected_chunks: usize,
) -> PhpResult<Vec<RawPacket>> {
    if input.len() > MAX_PROJECTION_BYTES {
        return Err(php_error(format!(
            "initial chunk projection exceeds {MAX_PROJECTION_BYTES} bytes"
        )));
    }

    let mut reader = ProjectionReader::new(input);
    let declared_chunks = usize::try_from(reader.read_u32_le()?)
        .map_err(|_| php_error("initial chunk count exceeds platform size"))?;
    if declared_chunks != expected_chunks {
        return Err(php_error(format!(
            "initial chunk projection count mismatch: expected {expected_chunks}, got {declared_chunks}"
        )));
    }
    if declared_chunks > MAX_INITIAL_CHUNKS {
        return Err(php_error(format!(
            "initial chunk projection exceeds {MAX_INITIAL_CHUNKS} chunks"
        )));
    }

    let mut packets = Vec::with_capacity(declared_chunks);
    for _ in 0..declared_chunks {
        let chunk_x = reader.read_i32_le()?;
        let chunk_z = reader.read_i32_le()?;
        let block_ids = reader.read_exact(CHUNK_BLOCK_COUNT)?;
        let block_data = reader.read_exact(CHUNK_NIBBLE_BYTES)?;
        let sky_light = reader.read_exact(CHUNK_NIBBLE_BYTES)?;
        let block_light = reader.read_exact(CHUNK_NIBBLE_BYTES)?;
        let biomes = reader.read_exact(CHUNK_COLUMN_COUNT)?;
        let height_map = reader.read_exact(CHUNK_COLUMN_COUNT)?;

        let extra_count = usize::try_from(reader.read_u32_le()?)
            .map_err(|_| php_error("chunk extra-data count exceeds platform size"))?;
        if extra_count > CHUNK_BLOCK_COUNT {
            return Err(php_error(
                "chunk extra-data count exceeds fixed-target block count",
            ));
        }
        let mut extra_data = Vec::with_capacity(extra_count);
        for _ in 0..extra_count {
            extra_data.push((reader.read_u32_le()?, reader.read_u16_le()?));
        }

        let snapshot = Protocol84ChunkSnapshot {
            chunk_x,
            chunk_z,
            block_ids,
            block_data,
            sky_light,
            block_light,
            biomes,
            height_map,
            extra_data: &extra_data,
        };
        packets.push(
            encode_protocol84_full_chunk_data(snapshot)
                .map_err(|error| php_error(error.to_string()))?,
        );
    }
    reader.finish()?;
    Ok(packets)
}
