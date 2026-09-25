# C003 design

## Runtime topology

A native scheduler owns N persistent PHP runtimes. Each runtime has bounded inbound command and completion queues. PHP-local plugin/runtime state never moves implicitly between runtimes. Native-backed identities may be referenced by handles, while mutation remains owner-gated.

## Message integrity

Every torture-run message carries a test sequence and logical producer/target identity so the harness can detect loss, duplication, corruption, and misrouting. Queue capacity is intentionally exceeded during saturation phases; the accepted behavior is explicit backpressure, never silent unbounded growth.

## Handle churn

The harness repeatedly creates, destroys, and reuses native slots while retaining stale handles for negative probes. Stale access must fail safely. Wrong-owner mutation tests run before, during, and after ownership transfer epochs.

## Completion and Fiber pressure

Native workers concurrently complete operations for multiple runtimes. Only the owning runtime drains its completion queue and resumes PHP Fibers. The harness records queue depth/latency and verifies shutdown cancellation semantics.

## GC and lifecycle

Cyclic GC stays enabled. The harness creates representative cyclic runtime-local PHP objects while message/completion traffic continues, recording GC/memory behavior. It repeatedly performs orderly shutdown and runtime restart to detect leaked thread/runtime state, deadlocks, stale completions, or use-after-destroy behavior.

## Gate

The output is a go/no-go decision with exact-revision evidence. Correctness/integrity, bounded behavior, and clean lifecycle are mandatory. Performance numbers are recorded, but no arbitrary production throughput threshold is invented before representative gameplay workloads exist.
