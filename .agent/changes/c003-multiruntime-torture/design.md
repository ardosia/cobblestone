# C003 design

## Runtime topology

A native scheduler owns N persistent PHP runtimes. Each runtime has bounded inbound command and completion queues. PHP-local plugin/runtime state never moves implicitly between runtimes. Native-backed identities may be referenced by handles, while mutation remains owner-gated.

## Initial runtime substrate

The first C003 substrate is deliberately process-isolated. The test-only `cobblestone-runtime-probe` crate under `tests/` owns persistent controlled PHP 8.5 ZTS subprocesses through a Cobblestone-owned `RuntimeProcess` abstraction. Each child keeps its Zend heap, cyclic GC state, and arbitrary PHP execution local to that runtime process. This harness is not a server CLI.

Each `RuntimeProcess` owns a bounded Rust command mailbox and bounded completion mailbox around one transport thread. Submission is nonblocking. Runtime transport batches are bounded, so protocol batching does not turn the mailbox into unbounded buffering. If completion draining stops, completion delivery eventually stalls command draining and surfaces explicit command-queue backpressure instead of allowing silent growth. Shutdown drops external mailbox endpoints, drains already accepted commands inside the transport thread, sends an explicit stop command, waits for the PHP process, and joins the transport thread.

This is a C003 experiment substrate, not a public plugin API or a final production topology decision. It intentionally establishes a conservative cross-platform isolation baseline before considering a same-process embedded or threaded Zend substrate. Process isolation does not prove shared-address-space PHP runtimes safe; later C003 evidence must account for that limitation when the go/no-go decision is recorded.

## Message integrity

Every torture-run message carries a test sequence, logical producer/target identity, and deterministic checksum so the harness can detect loss, duplication, corruption, and misrouting. The message torture routes at least one million logical messages across two runtime mailboxes while deliberately withholding completion draining until bounded command/completion pressure surfaces `Full`. The harness then drains and validates completions before continuing. Saturation must produce explicit backpressure; silent unbounded growth is not accepted.

## Handle churn and ownership epochs

C003 originally proved owner/epoch behavior with a private compatibility arena layered over the generational `Arena`. C004 subsequently productionized those semantics as `OwnedArena` in `cobblestone-core`; the maintained torture harness now exercises that production mechanism directly instead of keeping a duplicate wrapper.

The handle torture repeatedly creates, mutates, transfers, removes, and reuses native slots. It retains stale handles after reuse, requires every stale probe to remain rejected, verifies generation changes on slot reuse, requires wrong-owner mutations to fail before and after transfer, and requires stale-epoch commands from a prior transfer epoch to fail.

## Completion and GC pressure

The completion/GC torture keeps two persistent PHP runtimes alive with cyclic GC enabled while a bounded native `WorkerPool` concurrently executes owner-tagged work for both runtime identities. Every native completion is matched back to its accepted task identity and owner and carries an integrity checksum. PHP GC commands remain runtime-local and report cycle and memory observations through the bounded runtime completion path.

The harness records worker and runtime backpressure observations, maximum live native/GC work tracked by the harness, total completion counts per owner, GC completions/cycles, maximum observed PHP memory, end-to-end native completion latency, GC command round-trip latency, elapsed time, and whether PHP GC completions were observed while native work was still outstanding. Latency is recorded as sample count, average nanoseconds, and maximum nanoseconds so exact-revision evidence can compare platforms without inventing a production threshold.

C002 already proves the owner-runtime PHP Fiber wake path through the extension boundary. The process-isolated C003 harness does not duplicate that adapter API; it stresses the multi-runtime routing and lifecycle assumptions around the same bounded native completion mechanism.

## Cancellation, shutdown, and restart

A deterministic native cancellation probe blocks one worker, queues a second task, cancels it before execution, then requires the explicit `Cancelled` completion and a clean worker shutdown.

After the concurrent completion/GC phase, both PHP runtimes shut down cleanly. The lifecycle torture then repeatedly recreates both runtime identities, routes unique per-round messages and GC requests, validates exact per-round completions, and shuts both runtimes down again. Any completion from a prior instance therefore appears as an integrity failure instead of being executed silently.

## GC and lifecycle

Cyclic GC stays enabled. The harness creates representative cyclic runtime-local PHP objects while native completion traffic continues, recording GC/memory behavior. Repeated orderly shutdown and restart probes for leaked runtime state, deadlock, stale completion execution, and use-after-destroy behavior.

## Gate

The output is a go/no-go decision with exact-revision evidence. Correctness/integrity, bounded behavior, and clean lifecycle are mandatory. Performance numbers are recorded, but no arbitrary production throughput threshold is invented before representative gameplay workloads exist.
