use std::hint::black_box;
use std::time::{Duration, Instant};

use cobblestone_world::{ChunkCoord, ChunkPatch, WorldStore};

fn ns_per_op(elapsed: Duration, iterations: usize) -> f64 {
    elapsed.as_secs_f64() * 1_000_000_000.0 / iterations as f64
}

fn bench_world_change_journal() {
    const PATCHES: usize = 100_000;

    let store = WorldStore::new();
    let position = ChunkCoord::new(0, 0);
    assert!(store.ensure_chunk(position, 1));

    let started = Instant::now();
    for revision in 0..PATCHES {
        let revision = revision as u64;
        store
            .apply_patch(
                position,
                ChunkPatch {
                    expected_terrain_revision: revision,
                    next_terrain_revision: revision + 1,
                    expected_light_revision: 0,
                    next_light_revision: 0,
                    blocks: vec![(
                        u16::try_from(revision as usize % (16 * 16 * 128))
                            .expect("fixed-target index fits u16"),
                        if revision & 1 == 0 { 0x10 } else { 0x20 },
                    )],
                    ..ChunkPatch::default()
                },
            )
            .expect("point patch");
    }
    let elapsed = started.elapsed();

    println!(
        "world_bench name=world_point_patch_journal iterations={PATCHES} total_ns={} ns_per_op={:.2}",
        elapsed.as_nanos(),
        ns_per_op(elapsed, PATCHES),
    );

    const SEQUENCE_READS: usize = 1_000_000;
    let started = Instant::now();
    for _ in 0..SEQUENCE_READS {
        black_box(store.current_change_sequence());
    }
    let elapsed = started.elapsed();
    println!(
        "world_bench name=world_change_sequence iterations={SEQUENCE_READS} total_ns={} ns_per_op={:.2}",
        elapsed.as_nanos(),
        ns_per_op(elapsed, SEQUENCE_READS),
    );

    let latest = store.current_change_sequence();
    store.prune_changes_through(latest.saturating_sub(256));
    const SMALL_SNAPSHOTS: usize = 5_000;
    let started = Instant::now();
    for _ in 0..SMALL_SNAPSHOTS {
        black_box(store.change_log_snapshot());
    }
    let elapsed = started.elapsed();
    println!(
        "world_bench name=world_change_log_snapshot entries=256 iterations={SMALL_SNAPSHOTS} total_ns={} ns_per_op={:.2}",
        elapsed.as_nanos(),
        ns_per_op(elapsed, SMALL_SNAPSHOTS),
    );
}

fn main() {
    bench_world_change_journal();
}
