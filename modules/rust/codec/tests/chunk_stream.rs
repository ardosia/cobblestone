use cobblestone_codec::{
    CHUNK_BLOCK_COUNT, CHUNK_COLUMN_COUNT, CHUNK_NIBBLE_BYTES, CHUNK_ORDER_LAYERED, CodecError,
    FULL_CHUNK_DATA_ID, Protocol84ChunkSnapshot, encode_protocol84_full_chunk_data,
};

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

fn default_flat_planes() -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
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
    let block_light = vec![0_u8; CHUNK_NIBBLE_BYTES];
    let biomes = vec![1_u8; CHUNK_COLUMN_COUNT];
    let height_map = vec![3_u8; CHUNK_COLUMN_COUNT];

    (
        block_ids,
        block_data,
        sky_light,
        block_light,
        biomes,
        height_map,
    )
}

#[test]
fn real_default_flat_chunk_encodes_historical_layered_layout() {
    let (block_ids, block_data, sky_light, block_light, biomes, height_map) =
        default_flat_planes();

    let packet = encode_protocol84_full_chunk_data(Protocol84ChunkSnapshot {
        chunk_x: 8,
        chunk_z: -3,
        block_ids: &block_ids,
        block_data: &block_data,
        sky_light: &sky_light,
        block_light: &block_light,
        biomes: &biomes,
        height_map: &height_map,
        extra_data: &[],
    })
    .expect("encode default flat chunk");

    assert_eq!(packet.id(), FULL_CHUNK_DATA_ID);
    let body = packet.body().as_slice();
    assert_eq!(body.len(), 83_217);
    assert_eq!(&body[0..4], &8_i32.to_be_bytes());
    assert_eq!(&body[4..8], &(-3_i32).to_be_bytes());
    assert_eq!(body[8], CHUNK_ORDER_LAYERED);
    assert_eq!(u32::from_be_bytes(body[9..13].try_into().unwrap()), 83_204);

    let payload = &body[13..];
    // Layered protocol-84 terrain is X/Z/Y, so one vertical column is contiguous.
    assert_eq!(&payload[0..5], &[7, 3, 3, 2, 0]);
    assert_eq!(payload[2_048], 7); // x=1,z=0,y=0
    assert_eq!(payload[128], 7); // x=0,z=1,y=0

    let sky_offset = CHUNK_BLOCK_COUNT + CHUNK_NIBBLE_BYTES;
    let sky = &payload[sky_offset..sky_offset + CHUNK_NIBBLE_BYTES];
    assert_eq!(read_nibble(sky, 3), 0);
    assert_eq!(read_nibble(sky, 4), 15);
    assert_eq!(read_nibble(sky, 127), 15);

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
    let (block_ids, block_data, sky_light, block_light, biomes, height_map) =
        default_flat_planes();
    let extra = [(0x0000_ff7f_u32, 0xbeef_u16)];

    let packet = encode_protocol84_full_chunk_data(Protocol84ChunkSnapshot {
        chunk_x: 0,
        chunk_z: 0,
        block_ids: &block_ids,
        block_data: &block_data,
        sky_light: &sky_light,
        block_light: &block_light,
        biomes: &biomes,
        height_map: &height_map,
        extra_data: &extra,
    })
    .expect("encode sparse extra data");

    let payload = &packet.body().as_slice()[13..];
    let extra_offset =
        CHUNK_BLOCK_COUNT + CHUNK_NIBBLE_BYTES * 3 + CHUNK_COLUMN_COUNT + CHUNK_COLUMN_COUNT * 4;
    assert_eq!(&payload[extra_offset..extra_offset + 4], &[1, 0, 0, 0]);
    assert_eq!(
        &payload[extra_offset + 4..extra_offset + 8],
        &[0x7f, 0xff, 0x00, 0x00]
    );
    assert_eq!(
        &payload[extra_offset + 8..extra_offset + 10],
        &[0xef, 0xbe]
    );
}

#[test]
fn malformed_planes_and_unsupported_biomes_fail_explicitly() {
    let (_, block_data, sky_light, block_light, biomes, height_map) = default_flat_planes();
    let short_blocks = vec![0_u8; CHUNK_BLOCK_COUNT - 1];

    assert_eq!(
        encode_protocol84_full_chunk_data(Protocol84ChunkSnapshot {
            chunk_x: 0,
            chunk_z: 0,
            block_ids: &short_blocks,
            block_data: &block_data,
            sky_light: &sky_light,
            block_light: &block_light,
            biomes: &biomes,
            height_map: &height_map,
            extra_data: &[],
        }),
        Err(CodecError::InvalidChunkPlaneLength {
            field: "chunk block ids",
            expected: CHUNK_BLOCK_COUNT,
            actual: CHUNK_BLOCK_COUNT - 1,
        })
    );

    let (block_ids, block_data, sky_light, block_light, mut biomes, height_map) =
        default_flat_planes();
    biomes[0] = 255;
    assert_eq!(
        encode_protocol84_full_chunk_data(Protocol84ChunkSnapshot {
            chunk_x: 0,
            chunk_z: 0,
            block_ids: &block_ids,
            block_data: &block_data,
            sky_light: &sky_light,
            block_light: &block_light,
            biomes: &biomes,
            height_map: &height_map,
            extra_data: &[],
        }),
        Err(CodecError::UnsupportedChunkBiome { id: 255 })
    );
}
