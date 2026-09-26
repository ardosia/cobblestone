# C003 design

## Runtime topology

A native scheduler owns N persistent PHP runtimes. Each runtime has bounded inbound command and completion queues. PHP-local plugin/runtime state never moves implicitly between runtimes. Native-backed identities may be referenced by handles, while mutation remains owner-gated.

## Initial runtime substrate

The first C003 substrate is deliberately process-isolated. The private `cobblestone-runtime-probe` Rust crate owns persistent controlled PHP 8.5 ZTS CLI children through a Cobblestone-owned `RuntimeProcess` abstraction. Each child keeps its Zend heap, cyclic GC state, and arbitrary PHP execution local to that runtime process.

Each `RuntimeProcess` owns a bounded Rust command mailbox and bounded completion mailbox around one transport thread. Submission is nonblocking. If completion draining stops, completion delivery blocks the transport thread, which stops draining commands and eventually surfaces explicit command-queue backpressure instead of allowing unbounded growth. Shutdown drops external mailbox endpoints, drains already accepted commands inside the transport thread, sends an explicit stop command, waits for the PHP process, and joins the transport thread.

The transport preserves one bounded queue entry per logical command but may amortize pipe syscalls by draining up to 64 already-accepted commands into one buffered write/flush batch. PHP emits one integrity-checkable reply per command, followed by a batch acknowledgement, and flushes once per batch. This batching changes transport mechanics only; it does not turn many logical messages into one queue item or weaken per-message validation/backpressure.

This is a C003 experiment substrate, not a public plugin API or a final production topology decision. It intentionally establishes a conservative cross-platform isolation baseline before considering a same-process embedded or threaded Zend substrate. Process isolation does not prove shared-address-space PHP runtimes safe; later C003 evidence must account for that limitation when the go/no-go decision is recorded.

## Message integrity

Every torture-run message carries a test sequence, logical producer/target identity, and deterministic checksum so the harness can detect loss, duplication, corruption, and misrouting. The message torture routes at least one million logical messages across two runtime mailboxes while deliberately withholding completion draining until bounded command/completion pressure surfaces `Full`. The harness then drains and validates completions before continuing. Saturation must produce explicit backpressure; silent unbounded growth is not accepted.

The initial per-command pipe-flush implementation was intentionally measured rather than accepted by assumption. On the Windows CI runner it hit the deadlock watchdog after 600 seconds with 214,727 messages submitted and 214,621 completions received. That evidence motivated transport-level batching while preserving the same bounded logical-message queues and integrity oracle. The watchdog is a deadlock/test-bounding mechanism, not a production throughput threshold.

## Handle churn

C003 uses a private `OwnedProbeArena` layered over the proven generational `Arena`. Each probe has exactly one `RuntimeId` owner and an ownership epoch. Mutation and removal require both the current owner and current epoch. Transfer increments the epoch before the new owner can mutate; delayed commands carrying the old epoch fail safely.

The handle torture repeatedly creates, mutates, transfers, removes, and reuses native slots. It retains stale handles after reuse, requires every stale probe to remain rejected, verifies generation changes on slot reuse, requires wrong-owner mutations to fail before and after transfer, and requires stale-epoch commands from a prior transfer epoch to fail. This is an experiment surface for C003, not the final C004 ownership API.

## Completion and Fiber pressure

Native workers concurrently complete operations for multiple runtimes. Only the owning runtime drains its completion queue and resumes PHP Fibers. The harness records queue depth/latency and verifies shutdown cancellation semantics.

## GC and lifecycle

Cyclic GC stays enabled. The harness creates representative cyclic runtime-local PHP objects while message/completion traffic continues, recording GC/memory behavior. It repeatedly performs orderly shutdown and runtime restart to detect leaked thread/runtime state, deadlocks, stale completions, or use-after-destroy behavior.

## Gate

The output is a go/no-go decision with exact-revision evidence. Correctness/integrity, bounded behavior, and clean lifecycle are mandatory. Performance numbers are recorded, but no arbitrary production throughput threshold is invented before representative gameplay workloads exist.
