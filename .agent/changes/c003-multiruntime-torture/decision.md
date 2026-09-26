# C003 go/no-go decision framework

Status: **pending exact-revision platform validation**

C003 answers one bounded question: whether Cobblestone may continue toward production multi-runtime gameplay ownership using the proven persistent PHP-runtime substrate. It does not authorize arbitrary Zend access from native workers and it does not claim that multiple Zend runtimes are safe inside one shared process.

## Decision scope

The current prototype uses process-isolated persistent PHP 8.5 ZTS runtimes behind a Cobblestone-owned native abstraction. A successful C003 decision therefore applies to that isolation model and the ownership/routing contracts proven by the prototype.

Same-process or cross-thread embedded Zend runtimes remain outside the evidence boundary. Adopting that substrate later would require its own proof before gameplay depends on it.

## Required evidence

A GO decision requires exact-revision evidence for all of the following:

- at least two persistent PHP runtimes active concurrently;
- one million or more routed messages with no loss, duplication, corruption, or misrouting;
- observable bounded-queue backpressure under forced saturation;
- generational handle reuse with stale-handle rejection;
- wrong-owner rejection before and after ownership transfer;
- stale ownership-epoch rejection after transfer;
- owner-tagged native completions delivered without native workers invoking Zend;
- PHP cyclic GC exercised while completion/message pressure is active;
- deterministic cancellation of queued native work;
- clean worker/runtime shutdown without deadlock;
- repeated runtime restart without stale completion execution or leaked runtime identity;
- Linux and Windows execution where the controlled PHP 8.5 ZTS substrate is available.

Performance and latency are recorded as measurements. C003 does not invent a player-count, throughput, or latency threshold before representative gameplay exists.

## Outcome rules

**GO — process-isolated multi-runtime substrate:** every correctness/lifecycle requirement above is satisfied on the required platforms. C004 may then finalize production ownership metadata, and C011 may later introduce internal gameplay distribution after its other dependencies are complete.

**NO-GO:** the substrate demonstrates corruption, unbounded behavior, unsafe ownership, unreliable shutdown/restart, or platform instability that cannot be corrected within the bounded prototype. Distributed gameplay remains blocked while an alternative substrate is investigated.

**BLOCKED:** required evidence did not actually run or is inconclusive. A blocked result is not converted into GO by source inspection, generated tests, historical CI, or assumptions.

## Invariants after GO

Even after a GO decision:

- one mutable authoritative game object has exactly one owning PHP runtime;
- stable handles provide identity, not cross-runtime mutation permission;
- native workers carry owned/native data and never invoke arbitrary Zend APIs;
- runtime/region/thread ceremony remains absent from normal plugin APIs;
- network-shard ownership remains independent from gameplay-runtime ownership;
- C003 remains an experiment proof, not the final C004/C011 production API.
