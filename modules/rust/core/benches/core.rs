use std::hint::black_box;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use cobblestone_core::{
    Arena, ChunkCoord, ChunkPatch, Completion, NativeBuffer, TrySubmitError, WorkerPool, WorldStore,
};

#[derive(Debug)]
struct BenchValue(u64);

fn ns_per_op(elapsed: Duration, iterations: usize) -> f64 {
    elapsed.as_secs_f64() * 1_000_000_000.0 / iterations as f64
}

fn bench_handle_lookup() {
    const ITERATIONS: usize = 1_000_000;

    let mut arena = Arena::new();
    let handle = arena.insert(BenchValue(0xC0BB1E)).expect("arena slot");

    let started = Instant::now();
    for _ in 0..ITERATIONS {
        let value = arena.get(handle).expect("live handle");
        black_box(value.0);
    }
    let elapsed = started.elapsed();

    println!(
        "core_bench name=handle_lookup iterations={ITERATIONS} total_ns={} ns_per_op={:.2}",
        elapsed.as_nanos(),
        ns_per_op(elapsed, ITERATIONS),
    );
}

fn bench_buffer_copy() {
    const CASES: &[(usize, usize)] = &[
        (0, 50_000),
        (64, 20_000),
        (1_024, 5_000),
        (16_384, 500),
        (65_536, 100),
    ];

    for &(size, iterations) in CASES {
        let source = vec![0xA5; size];
        let started = Instant::now();

        for _ in 0..iterations {
            let buffer = NativeBuffer::copy_from_slice(black_box(&source));
            black_box(buffer.len());
        }

        let elapsed = started.elapsed();
        let explicit_bytes = size as u128 * iterations as u128;

        println!(
            "core_bench name=buffer_copy size={size} iterations={iterations} explicit_bytes={explicit_bytes} total_ns={} ns_per_op={:.2}",
            elapsed.as_nanos(),
            ns_per_op(elapsed, iterations),
        );
    }
}

fn bench_worker_submission() {
    const ITERATIONS: usize = 10_000;

    let pool = WorkerPool::new(2, ITERATIONS, ITERATIONS, |job: u64, _| job.wrapping_add(1))
        .expect("worker pool");

    let started = Instant::now();
    for job in 0..ITERATIONS {
        pool.try_submit(job as u64).expect("benchmark job accepted");
    }
    let elapsed = started.elapsed();

    let mut checksum = 0_u64;
    for _ in 0..ITERATIONS {
        match pool.recv_completion().expect("benchmark completion") {
            Completion::Completed { result, .. } => checksum = checksum.wrapping_add(result),
            Completion::Cancelled { .. } => panic!("benchmark job unexpectedly cancelled"),
            Completion::Panicked { .. } => panic!("benchmark job unexpectedly panicked"),
        }
    }
    black_box(checksum);

    let report = pool.shutdown();
    assert_eq!(report.worker_panics(), 0);
    assert!(report.completions().is_empty());

    println!(
        "core_bench name=worker_submit iterations={ITERATIONS} total_ns={} ns_per_op={:.2}",
        elapsed.as_nanos(),
        ns_per_op(elapsed, ITERATIONS),
    );
}

fn bench_queue_saturation() {
    const CAPACITY: usize = 64;

    let started = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let worker_started = Arc::clone(&started);
    let worker_release = Arc::clone(&release);

    let pool = WorkerPool::new(1, CAPACITY, CAPACITY + 1, move |job: u64, _| {
        worker_started.store(true, Ordering::Release);
        while !worker_release.load(Ordering::Acquire) {
            thread::yield_now();
        }
        job
    })
    .expect("worker pool");

    pool.try_submit(0).expect("blocking benchmark job accepted");
    while !started.load(Ordering::Acquire) {
        thread::yield_now();
    }

    let fill_started = Instant::now();
    for job in 1..=CAPACITY {
        pool.try_submit(job as u64)
            .expect("bounded queue slot accepted");
    }
    let fill_elapsed = fill_started.elapsed();

    match pool.try_submit((CAPACITY + 1) as u64) {
        Err(TrySubmitError::Full(job)) => {
            assert_eq!(job, (CAPACITY + 1) as u64);
        }
        other => panic!("expected explicit queue saturation, got {other:?}"),
    }

    release.store(true, Ordering::Release);
    let report = pool.shutdown();
    assert_eq!(report.worker_panics(), 0);
    assert_eq!(report.completions().len(), CAPACITY + 1);

    println!(
        "core_bench name=queue_saturation capacity={CAPACITY} fill_total_ns={} fill_ns_per_slot={:.2} full_result=observed",
        fill_elapsed.as_nanos(),
        ns_per_op(fill_elapsed, CAPACITY),
    );
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
        "core_bench name=world_point_patch_journal iterations={PATCHES} total_ns={} ns_per_op={:.2}",
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
        "core_bench name=world_change_sequence iterations={SEQUENCE_READS} total_ns={} ns_per_op={:.2}",
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
        "core_bench name=world_change_log_snapshot entries=256 iterations={SMALL_SNAPSHOTS} total_ns={} ns_per_op={:.2}",
        elapsed.as_nanos(),
        ns_per_op(elapsed, SMALL_SNAPSHOTS),
    );
}

fn main() {
    bench_handle_lookup();
    bench_buffer_copy();
    bench_worker_submission();
    bench_queue_saturation();
    bench_world_change_journal();
}
