# C003 design

## Runtime topology

A native scheduler owns N persistent PHP runtimes. Each runtime has bounded inbound command and completion queues. PHP-local plugin/runtime state never moves implicitly between runtimes. Native-backed identities may be referenced by handles, while mutation remains owner-gated.

## Initial runtime substrate

The first C003 substrate is deliberately process-isolated. The private `cobblestone-runtime-probe` Rust crate owns persistent controlled PHP 8.5 ZTS CLI children through a Cobblestone-owned `RuntimeProcess` abstraction. Each child keeps its Zend heap, cyclic GC state, and arbitrary PHP execution local to that runtime process.

Each `RuntimeProcess` owns a bounded Rust command mailbox and bounded completion mailbox around one transport thread. Submission is nonblocking. If completion draining stops, completion delivery blocks the transport thread, which stops draining commands and eventually surfaces explicit command-queue backpressure instead of allowing unbounded growth. Shutdown drops external mailbox endpoints, drains already accepted commands inside the transport thread, sends an explicit stop command, waits for the PHP process, and joins the transport thread.

This is a C003 experiment substrate, not a public plugin API or a final production topology decision. It intentionally establishes a conservative cross-platform isolation baseline before considering a same-process embedded or threaded Zend substrate. Process isolation does not prove shared-address-space PHP runtimes safe; later C003 evidence must account for that limitation when the go/no-go decision is recorded.

## Message integrity

Every torture-run message carries a test sequence, logical producer/target identity, and deterministic checksum so the harness can detect loss, duplication, corruption, and misrouting. The message torture routes at least one million logical messages across two runtime mailboxes while deliberately withholding completion draining until bounded command/completion pressure surfaces `Full`. The harness then drains and validates completions before continuing. Saturation must produce explicit backpressure; silent unbounded growth is not accepted.

The torture records elapsed time and observed backpressure events as measurements only. A generous deadlock watchdog bounds CI failure time but is not a production throughput threshold.

## Handle churn

The harness repeatedly creates, destroys, and reuses native slots while retaining stale handles for negative probes. Stale access must fail safely. Wrong-owner mutation tests run before, during, and after ownership transfer epochs.

## Completion and Fiber pressure

Native workers concurrently complete operations for multiple runtimes. Only the owning runtime drains its completion queue and resumes PHP Fibers. The harness records queue depth/latency and verifies shutdown cancellation semantics.

## GC and lifecycle

Cyclic GC stays enabled. The harness creates representative cyclic runtime-local PHP objects while message/completion traffic continues, recording GC/memory behavior. It repeatedly performs orderly shutdown and runtime restart to detect leaked thread/runtime state, deadlocks, stale completions, or use-after-destroy behavior.

## Gate

The output is a go/no-go decision with exact-revision evidence. Correctness/integrity, bounded behavior, and clean lifecycle are mandatory. Performance numbers are recorded, but no arbitrary production throughput threshold is invented before representative gameplay workloads exist.
