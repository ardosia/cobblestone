use std::hint::black_box;
use std::time::{Duration, Instant};

use cobblestone_protocol84::{
    BatchPacket, BootstrapPacket, CHUNK_BLOCK_COUNT, CHUNK_COLUMN_COUNT, CHUNK_NIBBLE_BYTES,
    CodecLimits, Protocol84ChunkSnapshot, RawPacket, UPDATE_BLOCK_FLAG_ALL_PRIORITY,
    encode_bootstrap_packet, encode_protocol84_full_chunk_data, encode_protocol84_update_block,
};
use cobblestone_target::ChunkShape;

fn limits() -> CodecLimits {
    CodecLimits::new(
        2 * 1024 * 1024,
        2 * 1024 * 1024,
        2 * 1024 * 1024,
        4 * 1024 * 1024,
        2 * 1024 * 1024,
        256,
    )
}

fn ns_per_op(elapsed: Duration, iterations: usize) -> f64 {
    elapsed.as_secs_f64() * 1_000_000_000.0 / iterations as f64
}

fn batch(packets: Vec<RawPacket>) -> RawPacket {
    encode_bootstrap_packet(&BootstrapPacket::Batch(BatchPacket::new(packets)), limits()).unwrap()
}

fn flat_chunk() -> RawPacket {
    let mut ids = vec![0_u8; CHUNK_BLOCK_COUNT];
    let data = vec![0_u8; CHUNK_NIBBLE_BYTES];
    for y in 0..4 {
        let id = if y == 0 {
            7
        } else if y < 3 {
            3
        } else {
            2
        };
        for z in 0..16 {
            for x in 0..16 {
                ids[(y << 8) | (z << 4) | x] = id;
            }
        }
    }
    let mut sky = vec![0_u8; CHUNK_NIBBLE_BYTES];
    for index in 4 * CHUNK_COLUMN_COUNT..CHUNK_BLOCK_COUNT {
        let byte = &mut sky[index >> 1];
        if index & 1 == 0 {
            *byte = (*byte & 0xf0) | 15;
        } else {
            *byte = (*byte & 0x0f) | 0xf0;
        }
    }
    let block = vec![0_u8; CHUNK_NIBBLE_BYTES];
    let biome_words = vec![0x0192_bc59_u32; CHUNK_COLUMN_COUNT];
    let heights = vec![3_u8; CHUNK_COLUMN_COUNT];
    encode_protocol84_full_chunk_data(Protocol84ChunkSnapshot {
        chunk_x: 0,
        chunk_z: 0,
        block_ids: &ids,
        block_data: &data,
        sky_light: &sky,
        block_light: &block,
        biome_words: &biome_words,
        height_map: &heights,
        extra_data: &[],
        block_entities: &[],
    })
    .unwrap()
}

fn noisy_chunk() -> RawPacket {
    let mut seed = 0x1234_5678_u32;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    let ids = (0..CHUNK_BLOCK_COUNT)
        .map(|_| next() as u8)
        .collect::<Vec<_>>();
    let data = (0..CHUNK_NIBBLE_BYTES)
        .map(|_| next() as u8)
        .collect::<Vec<_>>();
    let sky = (0..CHUNK_NIBBLE_BYTES)
        .map(|_| next() as u8)
        .collect::<Vec<_>>();
    let block = (0..CHUNK_NIBBLE_BYTES)
        .map(|_| next() as u8)
        .collect::<Vec<_>>();
    let biome_words = vec![0x0192_bc59_u32; CHUNK_COLUMN_COUNT];
    let heights = (0..CHUNK_COLUMN_COUNT)
        .map(|_| (next() & 0x7f) as u8)
        .collect::<Vec<_>>();
    encode_protocol84_full_chunk_data(Protocol84ChunkSnapshot {
        chunk_x: 0,
        chunk_z: 0,
        block_ids: &ids,
        block_data: &data,
        sky_light: &sky,
        block_light: &block,
        biome_words: &biome_words,
        height_map: &heights,
        extra_data: &[],
        block_entities: &[],
    })
    .unwrap()
}

fn updates(count: usize) -> Vec<RawPacket> {
    (0..count)
        .map(|i| {
            let x = (i % 16) as i32;
            let z = ((i / 16) % 16) as i32;
            let y = ((i / ChunkShape::COLUMN_COUNT) % ChunkShape::HEIGHT) as u8;
            let state = ((((i * 37) % 255) as u16 + 1) << 4) | (i as u16 & 0x0f);
            encode_protocol84_update_block(x, y, z, state, UPDATE_BLOCK_FLAG_ALL_PRIORITY).unwrap()
        })
        .collect()
}

fn measure(name: &str, packets: Vec<RawPacket>, iterations: usize) {
    let once = batch(packets.clone());
    let encoded = once.body().len() + 1;
    let start = Instant::now();
    for _ in 0..iterations {
        black_box(batch(packets.clone()));
    }
    let elapsed = start.elapsed();
    println!(
        "world_sync_bench name={name} packets={} encoded_bytes={encoded} iterations={iterations} ns_per_op={:.2}",
        packets.len(),
        ns_per_op(elapsed, iterations)
    );
}

fn main() {
    let flat = flat_chunk();
    let noisy = noisy_chunk();
    measure("full_flat", vec![flat], 2000);
    measure("full_noisy", vec![noisy], 200);
    for count in [1, 4, 8, 16, 32, 64, 128, 256] {
        measure(&format!("updates_{count}"), updates(count), 5000);
    }
}
