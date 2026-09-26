# C003 go/no-go decision

Status: **GO — process-isolated multi-runtime substrate**

Validated revision: `48654ca56a2f929231235312c27155dbac135860`

C003 answers one bounded question: whether Cobblestone may continue toward production multi-runtime gameplay ownership using the proven persistent PHP-runtime substrate. The answer is **GO for the tested process-isolated topology**.

The exact validated revision completed the Linux and Windows PHP-ZTS matrix together with the canonical Rust validation matrix. That matrix exercises two persistent runtime identities, at least one million integrity-checked routed messages with bounded-queue backpressure, generational handle reuse and stale rejection, wrong-owner and stale-epoch rejection across ownership transfer, owner-tagged native completion pressure, PHP cyclic GC, deterministic queued-task cancellation, clean shutdown, and repeated runtime restart.

## Decision scope

The tested substrate uses process-isolated persistent PHP 8.5 ZTS runtimes behind a Cobblestone-owned native abstraction. This decision authorizes C004 to productionize ownership metadata and allows later C011 work to depend on the process-isolated multi-runtime model after its other prerequisites are complete.

This decision does **not** establish that multiple embedded Zend runtimes are safe in one shared process or that arbitrary Zend objects may cross runtime/thread boundaries. A future same-process or cross-thread Zend substrate requires its own proof before gameplay depends on it.

## Required invariants

The GO decision preserves these constraints:

- one mutable authoritative game object has exactly one owning PHP runtime;
- stable handles provide identity, not cross-runtime mutation permission;
- wrong-owner mutation routes a semantic command or fails safely;
- native workers carry owned/native data and never invoke arbitrary Zend APIs;
- all cross-runtime/native queues remain bounded with explicit backpressure, cancellation, shutdown, and stale behavior;
- runtime/region/thread ceremony remains absent from normal plugin APIs;
- network-shard ownership remains independent from gameplay-runtime ownership;
- performance measurements are evidence, not invented production thresholds.

## Evidence boundary

The exact-revision CI result proves the required harnesses ran successfully on Ubuntu and Windows. The harnesses emit queue, memory, GC, completion-latency, and lifecycle observations during execution. C003 does not promote those synthetic measurements into player-count or production latency guarantees; representative gameplay/load evidence remains later work.

C003 is therefore closed as a successful experiment for the process-isolated topology, not as the final C004/C011 production API.
