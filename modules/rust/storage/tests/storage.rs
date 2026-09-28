use std::collections::BTreeMap;
use std::fs::{OpenOptions, remove_dir_all};
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use cobblestone_core::{
    CHUNK_BLOCK_COUNT, CHUNK_COLUMN_COUNT, CHUNK_LIFECYCLE_GENERATED,
    CHUNK_LIFECYCLE_LIGHT_POPULATED, CHUNK_LIFECYCLE_POPULATED, CHUNK_NIBBLE_BYTES, ChunkCoord,
    ChunkImport, ChunkPatch, WorldStore,
};
use cobblestone_storage::{
    Compression, CompressionPolicy, ExtensionSection, INDEX_PAGE_BYTES, REGION_HEADER_BYTES,
    RegionCoord, RegionFile, decode_chunk_record, encode_chunk_record,
    encode_chunk_record_with_policy,
};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);

fn temp_region_path(name: &str) -> PathBuf {
    let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir()
        .join(format!(
            "cobblestone-storage-{}-{name}-{serial}",
            std::process::id()
        ))
        .join("regions")
        .join("r.-1.1.cwr")
}

fn world_and_snapshots() -> (
    WorldStore,
    ChunkCoord,
    cobblestone_core::ChunkSnapshot,
    cobblestone_core::ChunkSnapshot,
) {
    let store = WorldStore::new();
    let position = ChunkCoord::new(-1, 17);
    store.ensure_chunk(position, 1);
    store.fill_layers(position, 0, 4, 0x10).unwrap();
    store.fill_sky_light_from(position, 4, 15).unwrap();
    store
        .set_lifecycle_flags(
            position,
            CHUNK_LIFECYCLE_GENERATED | CHUNK_LIFECYCLE_POPULATED | CHUNK_LIFECYCLE_LIGHT_POPULATED,
        )
        .unwrap();

    let index = (20_u16 << 8) | (3_u16 << 4) | 2_u16;
    store
        .apply_patch(
            position,
            ChunkPatch {
                expected_terrain_revision: 0,
                next_terrain_revision: 1,
                expected_light_revision: 0,
                next_light_revision: 1,
                blocks: vec![(index, 0x321)],
                biomes: vec![((4_u8 << 4) | 3_u8, 2)],
                extra_data: vec![(index, 0x1234)],
                block_light: vec![(index, 7)],
                ..ChunkPatch::default()
            },
        )
        .unwrap();
    let first = store.snapshot(position).unwrap();

    store
        .apply_patch(
            position,
            ChunkPatch {
                expected_terrain_revision: 1,
                next_terrain_revision: 2,
                expected_light_revision: 1,
                next_light_revision: 2,
                blocks: vec![(index, 0x451)],
                block_light: vec![(index, 8)],
                ..ChunkPatch::default()
            },
        )
        .unwrap();
    let second = store.snapshot(position).unwrap();

    (store, position, first, second)
}

fn noisy_snapshot() -> cobblestone_core::ChunkSnapshot {
    let store = WorldStore::new();
    let position = ChunkCoord::new(0, 0);
    let flags =
        CHUNK_LIFECYCLE_GENERATED | CHUNK_LIFECYCLE_POPULATED | CHUNK_LIFECYCLE_LIGHT_POPULATED;

    let mut seed = 0x1234_5678_u32;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };

    store
        .import_chunk(
            position,
            ChunkImport {
                terrain_revision: 7,
                light_revision: 3,
                lifecycle_flags: flags,
                states: (0..CHUNK_BLOCK_COUNT)
                    .map(|_| (next() & 0x0fff) as u16)
                    .collect(),
                sky_light: (0..CHUNK_NIBBLE_BYTES).map(|_| next() as u8).collect(),
                block_light: (0..CHUNK_NIBBLE_BYTES).map(|_| next() as u8).collect(),
                biomes: (0..CHUNK_COLUMN_COUNT).map(|_| next() as u8).collect(),
                height_map: (0..CHUNK_COLUMN_COUNT)
                    .map(|_| (next() & 0x7f) as u8)
                    .collect(),
                extra_data: BTreeMap::new(),
            },
        )
        .unwrap();
    store.snapshot(position).unwrap()
}

#[test]
fn adaptive_compression_keeps_only_material_wins() {
    let (_store, _position, flat, _) = world_and_snapshots();
    let noisy = noisy_snapshot();

    let flat_record =
        encode_chunk_record_with_policy(&flat, CompressionPolicy::Adaptive, &[]).unwrap();
    let noisy_record =
        encode_chunk_record_with_policy(&noisy, CompressionPolicy::Adaptive, &[]).unwrap();

    assert_eq!(
        decode_chunk_record(&flat_record).unwrap().compression,
        Compression::Zstd
    );
    assert_eq!(
        decode_chunk_record(&noisy_record).unwrap().compression,
        Compression::None
    );
}

#[test]
fn chunk_record_round_trips_semantics_and_extensions() {
    let (_store, position, snapshot, _) = world_and_snapshots();
    let extensions = vec![
        ExtensionSection {
            tag: 7,
            version: 1,
            payload: vec![1, 2, 3, 4],
        },
        ExtensionSection {
            tag: 9,
            version: 2,
            payload: vec![5; 32],
        },
    ];

    for compression in [Compression::None, Compression::Zstd] {
        let encoded = encode_chunk_record(&snapshot, compression, &extensions).unwrap();
        let decoded = decode_chunk_record(&encoded).unwrap();

        assert_eq!(decoded.position, position);
        assert_eq!(decoded.compression, compression);
        assert_eq!(decoded.import.terrain_revision, snapshot.terrain_revision());
        assert_eq!(decoded.import.light_revision, snapshot.light_revision());
        assert_eq!(decoded.import.lifecycle_flags, snapshot.lifecycle_flags());
        assert_eq!(decoded.import.states, snapshot.states());
        assert_eq!(decoded.import.sky_light, snapshot.sky_light());
        assert_eq!(decoded.import.block_light, snapshot.block_light());
        assert_eq!(decoded.import.biomes, snapshot.biomes());
        assert_eq!(decoded.import.height_map, snapshot.height_map());
        assert_eq!(decoded.import.extra_data, *snapshot.extra_data());
        assert_eq!(decoded.extensions, extensions);
    }
}

#[test]
fn region_commit_reopens_and_latest_valid_index_wins() {
    let (_store, position, first, second) = world_and_snapshots();
    let path = temp_region_path("reopen");
    let root = path
        .parent()
        .and_then(|regions| regions.parent())
        .unwrap()
        .to_path_buf();
    let world_uuid = [0x5a; 16];
    let coord = RegionCoord::for_chunk(position);

    {
        let mut region = RegionFile::open_or_create(&path, world_uuid, coord).unwrap();
        assert_eq!(region.generation(), 1);
        assert_eq!(
            region
                .save_chunk(&first, CompressionPolicy::Adaptive)
                .unwrap()
                .generation,
            2
        );
        assert_eq!(
            region
                .save_chunk(&second, CompressionPolicy::Adaptive)
                .unwrap()
                .generation,
            3
        );
        assert_eq!(region.indexed_chunks(), 1);
        let loaded = region.load_chunk(position).unwrap().unwrap();
        assert_eq!(loaded.import.terrain_revision, 2);
        assert_eq!(loaded.import.light_revision, 2);
        assert_eq!(loaded.import.states, second.states());
    }

    {
        let mut reopened = RegionFile::open_or_create(&path, world_uuid, coord).unwrap();
        assert_eq!(reopened.generation(), 3);
        let loaded = reopened.load_chunk(position).unwrap().unwrap();
        assert_eq!(loaded.import.terrain_revision, 2);
        assert_eq!(loaded.import.states, second.states());
    }

    remove_dir_all(root).unwrap();
}

#[test]
fn region_stats_measure_live_and_reclaimable_record_bytes() {
    let (_store, position, first, second) = world_and_snapshots();
    let path = temp_region_path("stats");
    let root = path
        .parent()
        .and_then(|regions| regions.parent())
        .unwrap()
        .to_path_buf();
    let world_uuid = [0x73; 16];
    let coord = RegionCoord::for_chunk(position);

    let mut region = RegionFile::open_or_create(&path, world_uuid, coord).unwrap();
    let empty = region.stats().unwrap();
    assert_eq!(empty.record_bytes, 0);
    assert_eq!(empty.live_bytes, 0);
    assert_eq!(empty.dead_bytes, 0);

    let first_save = region
        .save_chunk(&first, CompressionPolicy::Adaptive)
        .unwrap();
    assert_eq!(first_save.stats.record_bytes, first_save.bytes_appended);
    assert_eq!(first_save.stats.live_bytes, first_save.bytes_appended);
    assert_eq!(first_save.stats.dead_bytes, 0);
    assert_eq!(first_save.stats.indexed_chunks, 1);

    let second_save = region
        .save_chunk(&second, CompressionPolicy::Adaptive)
        .unwrap();
    assert_eq!(
        second_save.stats.record_bytes,
        first_save.bytes_appended + second_save.bytes_appended,
    );
    assert_eq!(second_save.stats.live_bytes, second_save.bytes_appended);
    assert_eq!(second_save.stats.dead_bytes, first_save.bytes_appended);
    assert_eq!(
        second_save.stats.record_bytes,
        second_save.stats.live_bytes + second_save.stats.dead_bytes,
    );

    drop(region);
    remove_dir_all(root).unwrap();
}

#[test]
fn torn_newest_index_falls_back_to_previous_valid_generation() {
    let (_store, position, first, second) = world_and_snapshots();
    let path = temp_region_path("fallback");
    let root = path
        .parent()
        .and_then(|regions| regions.parent())
        .unwrap()
        .to_path_buf();
    let world_uuid = [0xa5; 16];
    let coord = RegionCoord::for_chunk(position);

    {
        let mut region = RegionFile::open_or_create(&path, world_uuid, coord).unwrap();
        region
            .save_chunk(&first, CompressionPolicy::Adaptive)
            .unwrap();
        region
            .save_chunk(&second, CompressionPolicy::Adaptive)
            .unwrap();
        assert_eq!(region.generation(), 3);
    }

    // Generation 3 lives in page A after two commits (A=1, B=2, A=3).
    {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        file.seek(SeekFrom::Start(REGION_HEADER_BYTES as u64 + 128))
            .unwrap();
        file.write_all(&[0x7f]).unwrap();
        file.sync_data().unwrap();
    }

    {
        let mut recovered = RegionFile::open_or_create(&path, world_uuid, coord).unwrap();
        assert_eq!(recovered.generation(), 2);
        let loaded = recovered.load_chunk(position).unwrap().unwrap();
        assert_eq!(loaded.import.terrain_revision, 1);
        assert_eq!(loaded.import.light_revision, 1);
        assert_eq!(loaded.import.states, first.states());
    }

    remove_dir_all(root).unwrap();
}

#[test]
fn region_coordinates_are_storage_local_not_execution_region_sized() {
    assert_eq!(
        RegionCoord::for_chunk(ChunkCoord::new(-1, 17)),
        RegionCoord::new(-1, 1)
    );
    assert_eq!(INDEX_PAGE_BYTES, 16 * 1024);
}
