# Runtime foundation v1 design

## Intent

This round hardens the server runtime before the real world-to-protocol chunk stream lands. It does not change the fixed Minecraft 0.15.10 gameplay target.

The public design is intentionally clean rather than shaped like Mojang's C++ class graph. Fixed-target fidelity applies to observable gameplay semantics. Transaction, region, logging, and scheduling mechanisms are Cobblestone implementation choices.

## Logging

Application and plugin code depend on PSR-3. The default implementation is Monolog with a console formatter modeled after Spring Boot's default visual hierarchy:

`timestamp level pid --- [application] [execution] logger : message context`

There is no banner. Plugin code receives a scoped `LoggerInterface`, not Monolog types.

Native workers still do not invoke Zend/PHP logging APIs. A future native log bridge, if needed, must cross a bounded native queue and be drained by an owning runtime.

## Tick loop

The executable no longer owns a raw `while (true)`. `TickLoop` uses the monotonic clock, a fixed target period, explicit continuation control, and a warning threshold. When the server is late it logs both elapsed milliseconds and equivalent ticks behind. Warning emission is throttled.

After excessive backlog, the deadline is rebased instead of spinning indefinitely through obsolete catch-up deadlines. Gameplay tick callbacks themselves are never executed concurrently by this loop.

## Shutdown

`Server` has Starting/Running/Stopping/Stopped states and an idempotent stop path. A stop request causes the loop to finish the current tick and exit. On platforms with pcntl, SIGINT and SIGTERM become stop requests. A PHP shutdown hook is a final best-effort guard.

Shutdown continues through later phases after an earlier phase fails and rethrows the first failure after teardown. The order is stopping event, native session shutdown, plugin disable, then scheduler shutdown.

## World mutations

`World::mutate()` is a first-class functional gameplay surface. Its callback receives `WorldMutation`, an overlay where reads observe prior staged writes.

A mutation attempt records every touched chunk. If the discovered chunk set grows beyond the pre-resolved set, the attempt is discarded and replayed. Callbacks therefore must keep their side effects inside the mutation context. This prepares the same API for later region-owner routing.

Every touched chunk patch captures a base revision. All patches are prepared and revision-validated before the first authoritative write. Net-no-op and reverted writes do not advance revisions. A changed chunk advances exactly once per committed semantic mutation, regardless of how many cells/channels changed.

The current commit phase is atomic under the existing single owning PHP runtime. Cross-runtime commit coordination is deliberately not claimed yet.

Direct `Chunk` setters remain low-level generation/loading/commit primitives. Ordinary `World` mutation conveniences route through the coordinator.

## Regions

PHP owns the semantic mapping from chunks to execution regions. The default map is 8×8 chunks per execution region, explicitly a tunable Cobblestone mechanism rather than a fixed-target world fact or storage-region format.

Rust `cobblestone-core` owns `RegionDirectory`, which maps `RegionId` to exactly one `RuntimeId` plus `OwnershipEpoch`. Transfer advances the epoch and stale/wrong-owner routes are rejected.

Region mapping and routing are internal. Normal plugin APIs do not expose RegionId, RuntimeId, locks, queues, ownership epochs, or scheduler ceremony.

This round does not activate multiple gameplay runtimes. It establishes the seam so later region scheduling can be introduced without replacing the mutation API.