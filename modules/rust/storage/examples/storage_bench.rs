use std::collections::BTreeMap;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::Instant;

use cobblestone_core::{
    CHUNK_BLOCK_COUNT, CHUNK_COLUMN_COUNT, CHUNK_LIFECYCLE_GENERATED,
    CHUNK_LIFECYCLE_LIGHT_POPULATED, CHUNK_LIFECYCLE_POPULATED, CHUNK_NIBBLE_BYTES, ChunkCoord,
    ChunkImport, WorldStore,
};
use cobblestone_storage::{
    Compression, CompressionPolicy, RegionCoord, RegionFile, decode_chunk_record,
    encode_chunk_record, encode_chunk_record_with_policy,
};

fn snapshot(noisy: bool) -> cobblestone_core::ChunkSnapshot {
    let store = WorldStore::new();
    let position = ChunkCoord::new(0, 0);
    let flags =
        CHUNK_LIFECYCLE_GENERATED | CHUNK_LIFECYCLE_POPULATED | CHUNK_LIFECYCLE_LIGHT_POPULATED;

    let (states, sky_light, block_light, biomes, height_map) = if noisy {
        let mut seed = 0x1234_5678_u32;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        (
            (0..CHUNK_BLOCK_COUNT)
                .map(|_| (next() & 0x0fff) as u16)
                .collect(),
            (0..CHUNK_NIBBLE_BYTES).map(|_| next() as u8).collect(),
            (0..CHUNK_NIBBLE_BYTES).map(|_| next() as u8).collect(),
            (0..CHUNK_COLUMN_COUNT).map(|_| next() as u8).collect(),
            (0..CHUNK_COLUMN_COUNT)
                .map(|_| (next() & 0x7f) as u8)
                .collect(),
        )
    } else {
        let mut states = vec![0_u16; CHUNK_BLOCK_COUNT];
        for y in 0..4 {
            let state = match y {
                0 => 7 << 4,
                1 | 2 => 3 << 4,
                _ => 2 << 4,
            };
            let start = y * CHUNK_COLUMN_COUNT;
            states[start..start + CHUNK_COLUMN_COUNT].fill(state);
        }
        let mut sky = vec![0_u8; CHUNK_NIBBLE_BYTES];
        sky[(4 * CHUNK_COLUMN_COUNT) / 2..].fill(0xff);
        (
            states,
            sky,
            vec![0_u8; CHUNK_NIBBLE_BYTES],
            vec![1_u8; CHUNK_COLUMN_COUNT],
            vec![3_u8; CHUNK_COLUMN_COUNT],
        )
    };

    store
        .import_chunk(
            position,
            ChunkImport {
                terrain_revision: 17,
                light_revision: 9,
                lifecycle_flags: flags,
                states,
                sky_light,
                block_light,
                biomes,
                height_map,
                extra_data: BTreeMap::new(),
            },
        )
        .unwrap();
    store.snapshot(position).unwrap()
}

fn bench_codec(label: &str, snapshot: &cobblestone_core::ChunkSnapshot, iterations: usize) {
    for compression in [Compression::None, Compression::Zstd] {
        let encoded = encode_chunk_record(snapshot, compression, &[]).unwrap();

        let start = Instant::now();
        for _ in 0..iterations {
            black_box(encode_chunk_record(snapshot, compression, &[]).unwrap());
        }
        let encode_ns = start.elapsed().as_secs_f64() * 1e9 / iterations as f64;

        let start = Instant::now();
        for _ in 0..iterations {
            black_box(decode_chunk_record(&encoded).unwrap());
        }
        let decode_ns = start.elapsed().as_secs_f64() * 1e9 / iterations as f64;

        println!(
            "storage_bench kind=codec terrain={label} compression={compression:?} bytes={} encode_ns={encode_ns:.0} decode_ns={decode_ns:.0}",
            encoded.len()
        );
    }

    let encoded =
        encode_chunk_record_with_policy(snapshot, CompressionPolicy::Adaptive, &[]).unwrap();
    let selected = decode_chunk_record(&encoded).unwrap().compression;
    let start = Instant::now();
    for _ in 0..iterations {
        black_box(
            encode_chunk_record_with_policy(snapshot, CompressionPolicy::Adaptive, &[]).unwrap(),
        );
    }
    let encode_ns = start.elapsed().as_secs_f64() * 1e9 / iterations as f64;
    println!(
        "storage_bench kind=adaptive terrain={label} selected={selected:?} bytes={} encode_ns={encode_ns:.0}",
        encoded.len()
    );
}

fn bench_region(label: &str, snapshot: &cobblestone_core::ChunkSnapshot, iterations: usize) {
    let root: PathBuf = std::env::temp_dir().join(format!(
        "cobblestone-storage-bench-{}-{label}",
        std::process::id()
    ));
    let path = root.join("regions/r.0.0.cwr");
    let mut region = RegionFile::open_or_create(&path, [0x42; 16], RegionCoord::new(0, 0)).unwrap();

    let start = Instant::now();
    let mut appended = 0_u64;
    for _ in 0..iterations {
        appended += region
            .save_chunk(snapshot, CompressionPolicy::Adaptive)
            .unwrap()
            .bytes_appended;
    }
    let ns = start.elapsed().as_secs_f64() * 1e9 / iterations as f64;
    let stats = region.stats().unwrap();
    println!(
        "storage_bench kind=durable_commit terrain={label} iterations={iterations} avg_record_bytes={} ns_per_commit={ns:.0} live_bytes={} dead_bytes={} dead_ratio={:.3}",
        appended / iterations as u64,
        stats.live_bytes,
        stats.dead_bytes,
        if stats.record_bytes == 0 {
            0.0
        } else {
            stats.dead_bytes as f64 / stats.record_bytes as f64
        },
    );

    let compact_started = Instant::now();
    let compacted = region.compact().unwrap();
    let compact_ns = compact_started.elapsed().as_secs_f64() * 1e9;
    println!(
        "storage_bench kind=compaction terrain={label} records={} reclaimed_bytes={} before_bytes={} after_bytes={} ns={compact_ns:.0}",
        compacted.records,
        compacted.bytes_reclaimed,
        compacted.before.file_bytes,
        compacted.after.file_bytes,
    );

    drop(region);
    std::fs::remove_dir_all(root).unwrap();
}

fn main() {
    let flat = snapshot(false);
    let noisy = snapshot(true);
    bench_codec("flat", &flat, 1000);
    bench_codec("noisy", &noisy, 250);
    bench_region("flat", &flat, 50);
    bench_region("noisy", &noisy, 25);
}
