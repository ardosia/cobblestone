use std::hint::black_box;
use std::time::{Duration, Instant};

use cobblestone_runtime::Arena;

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
        "runtime_bench name=handle_lookup iterations={ITERATIONS} total_ns={} ns_per_op={:.2}",
        elapsed.as_nanos(),
        ns_per_op(elapsed, ITERATIONS),
    );
}

fn main() {
    bench_handle_lookup();
}
