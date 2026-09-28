use cobblestone_codec::{
    CHUNK_BLOCK_COUNT, CHUNK_COLUMN_COUNT, CHUNK_NIBBLE_BYTES, CHUNK_ORDER_LAYERED, CodecError,
    FULL_CHUNK_DATA_ID, Protocol84ChunkSnapshot, encode_protocol84_chunk_unload,
    encode_protocol84_full_chunk_data,
};

struct FlatChunkFixture {
    block_ids: Vec<u8>,
    block_data: Vec<u8>,
    sky_light: Vec<u8>,
    block_light: Vec<u8>,
    biomes: Vec<u8>,
    height_map: Vec<u8>,
}

impl FlatChunkFixture {
    fn default_world() -> Self {
        let mut block_ids = vec![0_u8; CHUNK_BLOCK_COUNT];
        for z in 0..16_usize {
            for x in 0..16_usize {
                block_ids[(z << 4) | x] = 7;
                block_ids[(1 << 8) | (z << 4) | x] = 3;
                block_ids[(2 << 8) | (z << 4) | x] = 3;
                block_ids[(3 << 8) | (z << 4) | x] = 2;
            }
        }

        let block_data = vec![0_u8; CHUNK_NIBBLE_BYTES];
        let mut sky_light = vec![0_u8; CHUNK_NIBBLE_BYTES];
        for semantic_index in (4 * 256)..CHUNK_BLOCK_COUNT {
            set_nibble(&mut sky_light, semantic_index, 15);
        }

        Self {
            block_ids,
            block_data,
            sky_light,
            block_light: vec![0_u8; CHUNK_NIBBLE_BYTES],
            biomes: vec![1_u8; CHUNK_COLUMN_COUNT],
            height_map: vec![3_u8; CHUNK_COLUMN_COUNT],
        }
    }

    fn snapshot<'a>(
        &'a self,
        chunk_x: i32,
        chunk_z: i32,
        extra_data: &'a [(u32, u16)],
    ) -> Protocol84ChunkSnapshot<'a> {
        Protocol84ChunkSnapshot {
            chunk_x,
            chunk_z,
            block_ids: &self.block_ids,
            block_data: &self.block_data,
            sky_light: &self.sky_light,
            block_light: &self.block_light,
            biomes: &self.biomes,
            height_map: &self.height_map,
            extra_data,
        }
    }
}

#[test]
fn protocol84_chunk_unload_is_client_managed_without_wire_packet() {
    assert!(encode_protocol84_chunk_unload(8, -3).is_none());
    assert!(encode_protocol84_chunk_unload(i32::MIN, i32::MAX).is_none());
}

fn set_nibble(bytes: &mut [u8], index: usize, value: u8) {
    let byte = &mut bytes[index >> 1];
    if index & 1 == 0 {
        *byte = (*byte & 0xf0) | value;
    } else {
        *byte = (*byte & 0x0f) | (value << 4);
    }
}

fn read_nibble(bytes: &[u8], index: usize) -> u8 {
    let byte = bytes[index >> 1];
    if index & 1 == 0 {
        byte & 0x0f
    } else {
        byte >> 4
    }
}

#[test]
fn real_default_flat_chunk_encodes_historical_layered_layout() {
    let fixture = FlatChunkFixture::default_world();

    let packet = encode_protocol84_full_chunk_data(fixture.snapshot(8, -3, &[]))
        .expect("encode default flat chunk");

    assert_eq!(packet.id(), FULL_CHUNK_DATA_ID);
    let body = packet.body().as_slice();
    assert_eq!(body.len(), 83_217);
    assert_eq!(&body[0..4], &8_i32.to_be_bytes());
    assert_eq!(&body[4..8], &(-3_i32).to_be_bytes());
    assert_eq!(body[8], CHUNK_ORDER_LAYERED);
    assert_eq!(u32::from_be_bytes(body[9..13].try_into().unwrap()), 83_204);

    let payload = &body[13..];
    // ORDER_LAYERED is Y/Z/X: each horizontal 16x16 layer is contiguous.
    assert_eq!(payload[0], 7);
    assert_eq!(payload[255], 7);
    assert_eq!(payload[256], 3);
    assert_eq!(payload[512], 3);
    assert_eq!(payload[768], 2);
    assert_eq!(payload[1_024], 0);

    let sky_offset = CHUNK_BLOCK_COUNT + CHUNK_NIBBLE_BYTES;
    let sky = &payload[sky_offset..sky_offset + CHUNK_NIBBLE_BYTES];
    assert_eq!(read_nibble(sky, 768), 0);
    assert_eq!(read_nibble(sky, 1_024), 15);
    assert_eq!(read_nibble(sky, CHUNK_BLOCK_COUNT - 1), 15);

    let height_offset = CHUNK_BLOCK_COUNT + CHUNK_NIBBLE_BYTES * 3;
    assert_eq!(payload[height_offset], 3);

    let biome_offset = height_offset + CHUNK_COLUMN_COUNT;
    assert_eq!(
        &payload[biome_offset..biome_offset + 4],
        &[0x01, 0x92, 0xbc, 0x59]
    );

    let extra_offset = biome_offset + CHUNK_COLUMN_COUNT * 4;
    assert_eq!(&payload[extra_offset..extra_offset + 4], &[0, 0, 0, 0]);
}

#[test]
fn sparse_extra_data_uses_historical_little_endian_entries() {
    let fixture = FlatChunkFixture::default_world();
    let extra = [(0x0000_ff7f_u32, 0xbeef_u16)];

    let packet = encode_protocol84_full_chunk_data(fixture.snapshot(0, 0, &extra))
        .expect("encode sparse extra data");

    let payload = &packet.body().as_slice()[13..];
    let extra_offset =
        CHUNK_BLOCK_COUNT + CHUNK_NIBBLE_BYTES * 3 + CHUNK_COLUMN_COUNT + CHUNK_COLUMN_COUNT * 4;
    assert_eq!(&payload[extra_offset..extra_offset + 4], &[1, 0, 0, 0]);
    assert_eq!(
        &payload[extra_offset + 4..extra_offset + 8],
        &[0x7f, 0xff, 0x00, 0x00]
    );
    assert_eq!(&payload[extra_offset + 8..extra_offset + 10], &[0xef, 0xbe]);
}

#[test]
fn malformed_planes_and_unsupported_biomes_fail_explicitly() {
    let fixture = FlatChunkFixture::default_world();
    let short_blocks = vec![0_u8; CHUNK_BLOCK_COUNT - 1];

    assert_eq!(
        encode_protocol84_full_chunk_data(Protocol84ChunkSnapshot {
            chunk_x: 0,
            chunk_z: 0,
            block_ids: &short_blocks,
            block_data: &fixture.block_data,
            sky_light: &fixture.sky_light,
            block_light: &fixture.block_light,
            biomes: &fixture.biomes,
            height_map: &fixture.height_map,
            extra_data: &[],
        }),
        Err(CodecError::InvalidChunkPlaneLength {
            field: "chunk block ids",
            expected: CHUNK_BLOCK_COUNT,
            actual: CHUNK_BLOCK_COUNT - 1,
        })
    );

    let mut fixture = FlatChunkFixture::default_world();
    fixture.biomes[0] = 255;
    assert_eq!(
        encode_protocol84_full_chunk_data(fixture.snapshot(0, 0, &[])),
        Err(CodecError::UnsupportedChunkBiome { id: 255 })
    );
}
