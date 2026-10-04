use std::collections::BTreeMap;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::Instant;

use cobblestone_storage::{
    Compression, CompressionPolicy, RegionCoord, RegionFile, decode_chunk_record,
    encode_chunk_record, encode_chunk_record_with_policy,
};
use cobblestone_world::{
    BLOCK_IDS, CHUNK_BLOCK_COUNT, CHUNK_COLUMN_COUNT, CHUNK_LIFECYCLE_GENERATED,
    CHUNK_LIFECYCLE_LIGHT_POPULATED, CHUNK_LIFECYCLE_POPULATED, CHUNK_NIBBLE_BYTES, ChunkCoord,
    ChunkImport, WorldStore,
};

fn snapshot(noisy: bool) -> cobblestone_world::ChunkSnapshot {
    snapshot_at(ChunkCoord::new(0, 0), noisy, 0x1234_5678)
}

fn snapshot_at(
    position: ChunkCoord,
    noisy: bool,
    noise_seed: u32,
) -> cobblestone_world::ChunkSnapshot {
    let store = WorldStore::new();
    let flags =
        CHUNK_LIFECYCLE_GENERATED | CHUNK_LIFECYCLE_POPULATED | CHUNK_LIFECYCLE_LIGHT_POPULATED;

    let (states, sky_light, block_light, biomes, height_map) = if noisy {
        let mut seed = noise_seed;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        (
            (0..CHUNK_BLOCK_COUNT)
                .map(|_| {
                    let value = next();
                    let block_id = BLOCK_IDS[(value as usize) % BLOCK_IDS.len()];
                    (u16::from(block_id) << 4) | ((value >> 8) as u16 & 0x0f)
                })
                .collect(),
            (0..CHUNK_NIBBLE_BYTES).map(|_| next() as u8).collect(),
            (0..CHUNK_NIBBLE_BYTES).map(|_| next() as u8).collect(),
            (0..CHUNK_COLUMN_COUNT)
                .map(|_| (1_u32 << 24) | (next() & 0x00ff_ffff))
                .collect(),
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
            vec![0x0192_bc59_u32; CHUNK_COLUMN_COUNT],
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

fn bench_codec(label: &str, snapshot: &cobblestone_world::ChunkSnapshot, iterations: usize) {
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

fn bench_region(label: &str, snapshot: &cobblestone_world::ChunkSnapshot, iterations: usize) {
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

fn bench_policy_churn() {
    const DEFAULT_MIN_DEAD_BYTES: u64 = 64 * 1024 * 1024;
    const HOT_CHUNKS: usize = 64;

    let root: PathBuf = std::env::temp_dir().join(format!(
        "cobblestone-storage-policy-bench-{}",
        std::process::id()
    ));
    let path = root.join("regions/r.0.0.cwr");
    let mut region = RegionFile::open_or_create(&path, [0x51; 16], RegionCoord::new(0, 0)).unwrap();

    let mut snapshots = Vec::with_capacity(256);
    for z in 0..16 {
        for x in 0..16 {
            let index = (z * 16 + x) as usize;
            let position = ChunkCoord::new(x, z);
            let noisy = index < HOT_CHUNKS;
            snapshots.push(snapshot_at(
                position,
                noisy,
                0x9e37_79b9_u32.wrapping_add(index as u32 * 0x85eb_ca6b),
            ));
        }
    }

    for snapshot in &snapshots {
        region
            .save_chunk(snapshot, CompressionPolicy::Adaptive)
            .unwrap();
    }

    let started = Instant::now();
    let mut rewrite_rounds = 0_usize;
    while region.stats().unwrap().dead_bytes < DEFAULT_MIN_DEAD_BYTES {
        for snapshot in &snapshots[..HOT_CHUNKS] {
            region
                .save_chunk(snapshot, CompressionPolicy::Adaptive)
                .unwrap();
        }
        rewrite_rounds += 1;
        assert!(
            rewrite_rounds <= 64,
            "policy benchmark failed to reach threshold"
        );
    }
    let churn_ns = started.elapsed().as_secs_f64() * 1e9;
    let before = region.stats().unwrap();

    let compact_started = Instant::now();
    let compacted = region.compact().unwrap();
    let compact_ns = compact_started.elapsed().as_secs_f64() * 1e9;

    println!(
        "storage_bench kind=policy_churn hot_chunks={HOT_CHUNKS} rewrite_rounds={rewrite_rounds} live_bytes={} dead_bytes={} dead_ratio={:.3} churn_ns={churn_ns:.0} reclaimed_bytes={} compact_ns={compact_ns:.0}",
        before.live_bytes,
        before.dead_bytes,
        before.dead_bytes as f64 / before.record_bytes as f64,
        compacted.bytes_reclaimed,
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
    bench_policy_churn();
}
