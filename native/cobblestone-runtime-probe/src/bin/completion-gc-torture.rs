use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{TryRecvError, TrySendError};
use std::thread;
use std::time::{Duration, Instant};

use cobblestone_core::{Completion, RuntimeId, TrySubmitError, WorkerPool};
use cobblestone_runtime_probe::{RoutedMessage, RuntimeCommand, RuntimeCompletion, RuntimeProcess};

const NATIVE_TASKS: u64 = 100_000;
const GC_INTERVAL: u64 = 1_024;
const RESTART_ROUNDS: u64 = 8;
const WATCHDOG: Duration = Duration::from_secs(300);

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct NativeJob {
    owner: RuntimeId,
    sequence: u64,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct NativeResult {
    owner: RuntimeId,
    sequence: u64,
    checksum: u64,
}

struct ExpectedNative {
    job: NativeJob,
    submitted_at: Instant,
}

#[derive(Default)]
struct LatencyStats {
    samples: u64,
    total_ns: u128,
    max_ns: u128,
}

impl LatencyStats {
    fn observe(&mut self, elapsed: Duration) {
        let elapsed_ns = elapsed.as_nanos();
        self.samples += 1;
        self.total_ns += elapsed_ns;
        self.max_ns = self.max_ns.max(elapsed_ns);
    }

    fn average_ns(&self) -> u128 {
        if self.samples == 0 {
            0
        } else {
            self.total_ns / u128::from(self.samples)
        }
    }
}

#[derive(Default)]
struct GcStats {
    completed: u64,
    cycles: u64,
    max_memory_bytes: u64,
    overlap_completions: u64,
    backpressure_events: u64,
    latency: LatencyStats,
}

fn main() -> Result<(), Box<dyn Error>> {
    let php = std::env::var_os("PHP_BINARY")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("php"));
    let worker = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tools/php-runtime-worker.php");

    let runtime_one = RuntimeId::new(1).ok_or_else(|| io::Error::other("runtime id 1 invalid"))?;
    let runtime_two = RuntimeId::new(2).ok_or_else(|| io::Error::other("runtime id 2 invalid"))?;

    let first = RuntimeProcess::spawn(runtime_one, &php, &worker, 64, 64)?;
    let second = RuntimeProcess::spawn(runtime_two, &php, &worker, 64, 64)?;

    let pool = WorkerPool::new(4, 128, 128, |job: NativeJob, _| NativeResult {
        owner: job.owner,
        sequence: job.sequence,
        checksum: native_checksum(job.sequence, job.owner),
    })?;

    let mut expected_native = HashMap::with_capacity(usize::try_from(NATIVE_TASKS)?);
    let mut expected_gc: HashMap<(u32, u64), Instant> = HashMap::new();
    let mut pending_gc_one = VecDeque::new();
    let mut pending_gc_two = VecDeque::new();
    let mut next_sequence = 1_u64;
    let mut next_gc_mark = GC_INTERVAL;
    let mut native_completed = 0_u64;
    let mut owner_one_completed = 0_u64;
    let mut owner_two_completed = 0_u64;
    let mut worker_backpressure_events = 0_u64;
    let mut max_native_inflight = 0_usize;
    let mut max_gc_inflight = 0_usize;
    let mut native_latency = LatencyStats::default();
    let mut gc_stats = GcStats::default();
    let started = Instant::now();

    while next_sequence <= NATIVE_TASKS
        || native_completed < NATIVE_TASKS
        || !pending_gc_one.is_empty()
        || !pending_gc_two.is_empty()
        || !expected_gc.is_empty()
    {
        if started.elapsed() > WATCHDOG {
            return Err(io::Error::other(format!(
                "completion/GC torture exceeded watchdog: next={next_sequence} native_completed={native_completed} pending_gc={} expected_gc={}",
                pending_gc_one.len() + pending_gc_two.len(),
                expected_gc.len()
            ))
            .into());
        }

        let mut made_progress = false;

        while next_sequence <= NATIVE_TASKS {
            let owner = if next_sequence & 1 == 0 {
                runtime_two
            } else {
                runtime_one
            };
            let job = NativeJob {
                owner,
                sequence: next_sequence,
            };

            match pool.try_submit(job) {
                Ok(handle) => {
                    let expected = ExpectedNative {
                        job,
                        submitted_at: Instant::now(),
                    };
                    if expected_native
                        .insert(handle.id().get(), expected)
                        .is_some()
                    {
                        return Err(io::Error::other("native task id was reused while live").into());
                    }
                    max_native_inflight = max_native_inflight.max(expected_native.len());
                    next_sequence += 1;
                    made_progress = true;

                    while next_sequence > next_gc_mark && next_gc_mark <= NATIVE_TASKS {
                        pending_gc_one.push_back(next_gc_mark * 2);
                        pending_gc_two.push_back(next_gc_mark * 2 + 1);
                        next_gc_mark += GC_INTERVAL;
                    }
                }
                Err(TrySubmitError::Full(returned)) => {
                    if returned != job {
                        return Err(io::Error::other(
                            "full worker queue returned a different native job",
                        )
                        .into());
                    }
                    worker_backpressure_events += 1;
                    break;
                }
                Err(TrySubmitError::Shutdown(_)) => {
                    return Err(io::Error::other("native worker pool shut down early").into());
                }
                Err(TrySubmitError::IdExhausted(_)) => {
                    return Err(io::Error::other("native worker task ids exhausted").into());
                }
            }
        }

        made_progress |= submit_pending_gc(
            &first,
            runtime_one,
            &mut pending_gc_one,
            &mut expected_gc,
            &mut gc_stats,
        )?;
        made_progress |= submit_pending_gc(
            &second,
            runtime_two,
            &mut pending_gc_two,
            &mut expected_gc,
            &mut gc_stats,
        )?;
        max_gc_inflight = max_gc_inflight.max(expected_gc.len());

        let drained = drain_native(
            &pool,
            &mut expected_native,
            &mut native_completed,
            &mut owner_one_completed,
            &mut owner_two_completed,
            &mut native_latency,
            runtime_one,
            runtime_two,
        )?;
        made_progress |= drained > 0;

        let first_gc = drain_gc(
            &first,
            runtime_one,
            &mut expected_gc,
            &mut gc_stats,
            native_completed,
        )?;
        let second_gc = drain_gc(
            &second,
            runtime_two,
            &mut expected_gc,
            &mut gc_stats,
            native_completed,
        )?;
        made_progress |= first_gc + second_gc > 0;

        if !made_progress {
            thread::yield_now();
        }
    }

    if !expected_native.is_empty()
        || native_completed != NATIVE_TASKS
        || owner_one_completed + owner_two_completed != NATIVE_TASKS
        || owner_one_completed == 0
        || owner_two_completed == 0
    {
        return Err(io::Error::other(format!(
            "native completion integrity failed: expected_live={} completed={native_completed} owner_one={owner_one_completed} owner_two={owner_two_completed}",
            expected_native.len()
        ))
        .into());
    }
    if native_latency.samples != NATIVE_TASKS {
        return Err(io::Error::other(format!(
            "native latency accounting mismatch: samples={} expected={NATIVE_TASKS}",
            native_latency.samples
        ))
        .into());
    }
    if gc_stats.completed == 0
        || gc_stats.overlap_completions == 0
        || gc_stats.latency.samples != gc_stats.completed
    {
        return Err(io::Error::other(format!(
            "PHP cyclic GC pressure evidence incomplete: gc_completed={} overlap={} latency_samples={}",
            gc_stats.completed, gc_stats.overlap_completions, gc_stats.latency.samples
        ))
        .into());
    }

    let report = pool.shutdown();
    if report.worker_panics() != 0 || !report.completions().is_empty() {
        return Err(io::Error::other(format!(
            "native worker shutdown left unexpected state: worker_panics={} drained_completions={}",
            report.worker_panics(),
            report.completions().len()
        ))
        .into());
    }

    run_cancellation_probe()?;

    let first_status = first.shutdown()?;
    let second_status = second.shutdown()?;
    if !first_status.success() || !second_status.success() {
        return Err(io::Error::other(format!(
            "completion/GC runtimes did not shut down cleanly: first={first_status:?} second={second_status:?}"
        ))
        .into());
    }

    run_restart_probe(&php, &worker, runtime_one, runtime_two)?;

    println!(
        "completion-gc-torture: native_tasks={NATIVE_TASKS} owner_one={owner_one_completed} owner_two={owner_two_completed} worker_backpressure_events={worker_backpressure_events} max_native_inflight={max_native_inflight} native_completion_avg_ns={} native_completion_max_ns={} gc_completions={} gc_cycles={} max_php_memory_bytes={} gc_overlap_completions={} runtime_backpressure_events={} max_gc_inflight={max_gc_inflight} gc_roundtrip_avg_ns={} gc_roundtrip_max_ns={} cancellation=verified restart_rounds={RESTART_ROUNDS} elapsed_ms={}",
        native_latency.average_ns(),
        native_latency.max_ns,
        gc_stats.completed,
        gc_stats.cycles,
        gc_stats.max_memory_bytes,
        gc_stats.overlap_completions,
        gc_stats.backpressure_events,
        gc_stats.latency.average_ns(),
        gc_stats.latency.max_ns,
        started.elapsed().as_millis()
    );
    Ok(())
}

fn submit_pending_gc(
    runtime: &RuntimeProcess,
    owner: RuntimeId,
    pending: &mut VecDeque<u64>,
    expected: &mut HashMap<(u32, u64), Instant>,
    stats: &mut GcStats,
) -> Result<bool, Box<dyn Error>> {
    let mut submitted = false;
    while let Some(&sequence) = pending.front() {
        match runtime.try_submit(RuntimeCommand::CollectGc { sequence }) {
            Ok(()) => {
                pending.pop_front();
                if expected
                    .insert((owner.get(), sequence), Instant::now())
                    .is_some()
                {
                    return Err(io::Error::other(format!(
                        "duplicate GC sequence scheduled: owner={} sequence={sequence}",
                        owner.get()
                    ))
                    .into());
                }
                submitted = true;
            }
            Err(TrySendError::Full(RuntimeCommand::CollectGc { sequence: returned })) => {
                if returned != sequence {
                    return Err(io::Error::other(
                        "full runtime mailbox returned a different GC command",
                    )
                    .into());
                }
                stats.backpressure_events += 1;
                break;
            }
            Err(TrySendError::Full(_)) => {
                return Err(io::Error::other(
                    "full runtime mailbox returned an unexpected command kind",
                )
                .into());
            }
            Err(TrySendError::Disconnected(_)) => {
                return Err(io::Error::other("runtime command mailbox disconnected").into());
            }
        }
    }
    Ok(submitted)
}

#[allow(clippy::too_many_arguments)]
fn drain_native(
    pool: &WorkerPool<NativeJob, NativeResult>,
    expected: &mut HashMap<u64, ExpectedNative>,
    completed: &mut u64,
    owner_one_completed: &mut u64,
    owner_two_completed: &mut u64,
    latency: &mut LatencyStats,
    runtime_one: RuntimeId,
    runtime_two: RuntimeId,
) -> Result<usize, Box<dyn Error>> {
    let mut drained = 0_usize;
    loop {
        match pool.try_recv_completion() {
            Ok(Completion::Completed { id, result }) => {
                let expected = expected.remove(&id.get()).ok_or_else(|| {
                    io::Error::other(format!(
                        "native completion referenced unknown task {}",
                        id.get()
                    ))
                })?;
                let job = expected.job;
                if result.owner != job.owner
                    || result.sequence != job.sequence
                    || result.checksum != native_checksum(job.sequence, job.owner)
                {
                    return Err(io::Error::other(format!(
                        "native completion was corrupted or misrouted: job={job:?} result={result:?}"
                    ))
                    .into());
                }
                latency.observe(expected.submitted_at.elapsed());
                if result.owner == runtime_one {
                    *owner_one_completed += 1;
                } else if result.owner == runtime_two {
                    *owner_two_completed += 1;
                } else {
                    return Err(io::Error::other("native completion had an unknown owner").into());
                }
                *completed += 1;
                drained += 1;
            }
            Ok(Completion::Cancelled { id }) => {
                return Err(io::Error::other(format!(
                    "ordinary completion stress unexpectedly cancelled task {}",
                    id.get()
                ))
                .into());
            }
            Ok(Completion::Panicked { id }) => {
                return Err(io::Error::other(format!(
                    "ordinary completion stress task {} panicked",
                    id.get()
                ))
                .into());
            }
            Err(TryRecvError::Empty) => return Ok(drained),
            Err(TryRecvError::Disconnected) => {
                return Err(io::Error::other("native completion queue disconnected early").into());
            }
        }
    }
}

fn drain_gc(
    runtime: &RuntimeProcess,
    owner: RuntimeId,
    expected: &mut HashMap<(u32, u64), Instant>,
    stats: &mut GcStats,
    native_completed: u64,
) -> Result<usize, Box<dyn Error>> {
    let mut drained = 0_usize;
    loop {
        match runtime.try_recv_completion() {
            Ok(RuntimeCompletion::Gc {
                sequence,
                cycles,
                memory_bytes,
            }) => {
                let submitted_at = expected.remove(&(owner.get(), sequence)).ok_or_else(|| {
                    io::Error::other(format!(
                        "unexpected or duplicate GC completion: owner={} sequence={sequence}",
                        owner.get()
                    ))
                })?;
                stats.completed += 1;
                stats.cycles += u64::from(cycles);
                stats.max_memory_bytes = stats.max_memory_bytes.max(memory_bytes);
                stats.latency.observe(submitted_at.elapsed());
                if native_completed < NATIVE_TASKS {
                    stats.overlap_completions += 1;
                }
                drained += 1;
            }
            Ok(other) => {
                return Err(io::Error::other(format!(
                    "unexpected runtime completion during GC pressure: {other:?}"
                ))
                .into());
            }
            Err(TryRecvError::Empty) => return Ok(drained),
            Err(TryRecvError::Disconnected) => {
                return Err(io::Error::other("runtime completion mailbox disconnected").into());
            }
        }
    }
}

fn run_cancellation_probe() -> Result<(), Box<dyn Error>> {
    let started = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let worker_started = Arc::clone(&started);
    let worker_release = Arc::clone(&release);

    let pool = WorkerPool::new(1, 2, 2, move |value: u64, _| {
        if value == 1 {
            worker_started.store(true, Ordering::Release);
            while !worker_release.load(Ordering::Acquire) {
                thread::yield_now();
            }
        }
        value
    })?;

    let first = pool
        .try_submit(1)
        .map_err(|_| io::Error::other("failed to submit blocking cancellation probe task"))?;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !started.load(Ordering::Acquire) {
        if Instant::now() >= deadline {
            release.store(true, Ordering::Release);
            return Err(io::Error::other("cancellation probe worker did not start").into());
        }
        thread::yield_now();
    }

    let second = match pool.try_submit(2) {
        Ok(handle) => handle,
        Err(_) => {
            release.store(true, Ordering::Release);
            return Err(io::Error::other("failed to queue cancellable probe task").into());
        }
    };
    second.cancel();
    release.store(true, Ordering::Release);

    match pool.recv_completion()? {
        Completion::Completed { id, result } if id == first.id() && result == 1 => {}
        other => {
            return Err(io::Error::other(format!(
                "unexpected first cancellation-probe completion: {other:?}"
            ))
            .into());
        }
    }
    match pool.recv_completion()? {
        Completion::Cancelled { id } if id == second.id() => {}
        other => {
            return Err(io::Error::other(format!(
                "queued task was not cancelled before execution: {other:?}"
            ))
            .into());
        }
    }

    let report = pool.shutdown();
    if report.worker_panics() != 0 || !report.completions().is_empty() {
        return Err(io::Error::other("cancellation probe shutdown was not clean").into());
    }
    Ok(())
}

fn run_restart_probe(
    php: &Path,
    worker: &Path,
    runtime_one: RuntimeId,
    runtime_two: RuntimeId,
) -> Result<(), Box<dyn Error>> {
    for round in 0..RESTART_ROUNDS {
        let first = RuntimeProcess::spawn(runtime_one, php, worker, 8, 8)?;
        let second = RuntimeProcess::spawn(runtime_two, php, worker, 8, 8)?;
        if first.boot().pid() == second.boot().pid() {
            return Err(io::Error::other("restart round runtimes share one process").into());
        }

        let base = 10_000_000_u64 + round * 16;
        let first_message = RoutedMessage {
            sequence: base,
            producer: runtime_one,
            target: runtime_one,
            checksum: lifecycle_checksum(base, runtime_one, runtime_one),
        };
        let second_message = RoutedMessage {
            sequence: base + 1,
            producer: runtime_one,
            target: runtime_two,
            checksum: lifecycle_checksum(base + 1, runtime_one, runtime_two),
        };

        first.try_submit(RuntimeCommand::Message(first_message))?;
        second.try_submit(RuntimeCommand::Message(second_message))?;
        expect_message(&first, first_message)?;
        expect_message(&second, second_message)?;

        first.try_submit(RuntimeCommand::CollectGc { sequence: base + 2 })?;
        second.try_submit(RuntimeCommand::CollectGc { sequence: base + 3 })?;
        expect_gc(&first, base + 2)?;
        expect_gc(&second, base + 3)?;

        let first_status = first.shutdown()?;
        let second_status = second.shutdown()?;
        if !first_status.success() || !second_status.success() {
            return Err(io::Error::other(format!(
                "restart round {round} did not shut down cleanly: first={first_status:?} second={second_status:?}"
            ))
            .into());
        }
    }
    Ok(())
}

fn expect_message(runtime: &RuntimeProcess, expected: RoutedMessage) -> Result<(), Box<dyn Error>> {
    match runtime.recv_completion_timeout(Duration::from_secs(10))? {
        RuntimeCompletion::Message(actual) if actual == expected => Ok(()),
        other => Err(io::Error::other(format!(
            "restart probe observed stale/corrupt message: expected={expected:?} actual={other:?}"
        ))
        .into()),
    }
}

fn expect_gc(runtime: &RuntimeProcess, sequence: u64) -> Result<(), Box<dyn Error>> {
    match runtime.recv_completion_timeout(Duration::from_secs(10))? {
        RuntimeCompletion::Gc {
            sequence: actual, ..
        } if actual == sequence => Ok(()),
        other => Err(io::Error::other(format!(
            "restart probe observed stale/corrupt GC completion for {sequence}: {other:?}"
        ))
        .into()),
    }
}

const fn native_checksum(sequence: u64, owner: RuntimeId) -> u64 {
    sequence.rotate_left(19) ^ ((owner.get() as u64) << 32) ^ 0x9e37_79b9_7f4a_7c15
}

const fn lifecycle_checksum(sequence: u64, producer: RuntimeId, target: RuntimeId) -> u64 {
    sequence.rotate_left(17)
        ^ ((producer.get() as u64) << 32)
        ^ (target.get() as u64)
        ^ 0xd6e8_feb8_6659_fd93
}
