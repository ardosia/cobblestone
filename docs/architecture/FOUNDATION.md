# Cobblestone foundation architecture

## Scope

Cobblestone is fixed initially to Minecraft Windows 10 Edition Beta / MCPE 0.15.10, game protocol 84, and RakNet protocol 8. PHP 8.5 ZTS is the controlled high-level runtime. Rust/native code supplies mechanisms and measured hot paths.

The governing split is:

> PHP owns gameplay semantics and the developer/plugin experience. Rust/native extensions own mechanisms, concurrency infrastructure, networking, native representations, and measured hot paths.

This excludes both a Rust server with PHP bolted on as a guest and a traditional single-threaded PHP server that exposes thread primitives to plugins.

## Ownership model

Mutable authoritative objects have one owning PHP runtime. Long-lived identities such as players, entities, worlds, sessions, and selected chunk state use native-backed generational handles. A handle provides identity only. Native ownership metadata determines where mutation is legal.

Wrong-owner behavior is semantic: either route a command to the owner or reject the operation. It must never become cross-runtime arbitrary Zend object access.

Immutable/native shared values such as packet buffers, snapshots, chunk snapshots, encoded blobs, and lookup/catalog data may cross concurrency boundaries without transferring mutable authority.

## Native module boundaries

Initial module families are:

- `cobblestone-core`: runtime identity, generational handles, bounded workers, completion/future plumbing, Fiber wake integration, cancellation, immutable buffers, telemetry, panic/error boundaries, and any deliberately versioned sibling-module ABI.
- `cobblestone-network`: protocol-8 RakNet state machines and network-shard orchestration. It does not know about Player, World, plugins, or gameplay regions.
- `cobblestone-codec`: protocol-84 binary codec, batch/compression, packet primitives, NBT where appropriate, and native-buffer integration.
- `cobblestone-world`: native world/chunk structures only where profiling/evidence justifies them, favoring snapshots and bulk operations.
- `cobblestone-storage`: introduced only when storage-native mechanisms justify a separate module.

Sibling modules must not reach into each other's private Rust structs or depend on unstable struct layouts.

## C001 — engineering bootstrap

C001 creates the durable project identity, fixed-target requirements, S3 parent program, concrete C001-C003 child changes, architecture/provenance docs, repository instructions, and validation/CI entrypoints. It does not claim any native implementation exists.

C001 is complete only after the resulting remote revision passes the repository metadata validation and repository state is re-read.

## C002 — PHP 8.5 ZTS plus `cobblestone-core` proof

C002 is a sequence of bounded proofs, not one large extension dump.

### C002 primitive slice

The first implementation slice creates a Rust workspace and a safe `cobblestone-core` crate containing:

- `RuntimeId` as an explicit nonzero runtime identity type;
- a type-safe generational handle/arena with stale-generation rejection and slot retirement on generation exhaustion;
- immutable reference-counted native buffers with explicit copy-in and cheap clone semantics.

This slice deliberately contains no Zend calls. It proves ownership primitives independently before FFI is introduced.

### C002 extension boundary slice

A following slice must load as a PHP 8.5 ZTS extension on Linux and Windows and expose only diagnostic/proof surfaces, not the final plugin API. The extension adapter must define PHP/Zend ownership and thread-affinity rules, panic containment, safe error conversion, allocation ownership, runtime identity attachment, and invalid/stale-handle handling.

The underlying Rust mechanism crate remains independently testable. No third-party PHP threading substrate becomes part of the public Cobblestone API.

### C002 worker/completion slice

The worker proof uses a bounded native pool. Workers accept immutable/owned inputs, never invoke arbitrary Zend APIs, and publish completions to an owning-runtime queue. Cancellation and shutdown are explicit. PHP Fibers may suspend while awaiting work, but a suspended Fiber never blocks the server thread.

### C002 measurement

Benchmarks must eventually cover empty PHP-to-native calls, handle lookup, native-buffer handoff sizes, worker submission, completion delivery/Fiber wake, queue saturation/backpressure, and bytes copied across the boundary. Performance conclusions remain pending until these benchmarks actually run.

## C003 — multi-runtime PHP torture prototype

C003 is a go/no-go experiment performed before gameplay is distributed across PHP runtimes.

### Prototype topology

A native scheduler owns a fixed set of persistent PHP runtimes. Each runtime has a bounded inbound command queue and a bounded completion queue. Runtime-local PHP state stays local. Native-backed identities can be referenced by stable handles, but mutation is accepted only by the current owner.

No public plugin API exposes runtime or region ceremony.

### Required stress cases

The prototype must exercise:

- at least two persistent PHP runtimes running concurrently;
- at least one million routed messages with sequence/integrity checks;
- bounded-queue saturation and observable backpressure;
- repeated handle create/destroy/reuse with stale-handle probes;
- concurrent native completions routed to owning runtimes;
- PHP cyclic GC under sustained message/completion pressure;
- orderly shutdown, cancellation, and runtime restart;
- wrong-owner commands during ownership changes;
- Linux and Windows where the PHP 8.5 ZTS substrate is available.

### Go/no-go evidence

C003 may proceed to a production ownership design only if the prototype demonstrates deterministic routing/integrity, safe stale-handle behavior, bounded memory/queues under saturation, clean shutdown/restart, and a substrate that remains stable under sustained concurrent execution. Throughput/latency are recorded as measurements, not converted into invented pass thresholds before a representative workload exists.

If the initial PHP multi-runtime substrate is unstable, Cobblestone investigates alternatives before gameplay depends on it.

## C005 — protocol-8 RakNet transport

C005 introduces `cobblestone-network` as the transport boundary. The implementation pins the exact Ardosia RakNet consumer revision recorded in `docs/provenance/ARDOSIA_REUSE.md` instead of following a moving transport branch.

### Transport boundary

`cobblestone-network` owns:

- UDP/RakNet listener and connection lifecycle;
- RakNet reliability selection;
- bounded backend command delivery;
- bounded per-connection inbound payload delivery;
- transport-level backpressure and disconnect behavior;
- clean transport shutdown.

It does not own game protocol 84, packet codecs, batch/compression, NBT, players, worlds, gameplay sessions, plugins, or gameplay ownership.

### Fixed compatibility profile

The initial public configuration is deliberately narrow. `NetworkConfig::protocol8` selects RakNet protocol 8 and disables the newer handshake-cookie path so the transport matches the accepted MCPE 0.15.10 target. The server advertisement is treated as an opaque transport string supplied by the layer above.

A generic protocol list or modern-Bedrock compatibility switch is not exposed. Expanding the transport target requires an accepted change.

### Bounded facade

Backend commands and per-peer inbound payloads use bounded queues. A peer that exhausts its inbound capacity is closed through an explicit backpressure terminal state instead of creating an unbounded application backlog. Transport ownership remains independent from gameplay-runtime ownership.

### Transport verification

C005 fixtures require raw protocol-8 Request1 acceptance, incompatible protocol rejection, completed protocol-8 connection establishment, bidirectional reliable-ordered payload flow, and reassembly of a payload large enough to exercise RakNet fragmentation.

These fixtures validate transport mechanics only. Protocol-84 packet compatibility belongs to C006 and later end-to-end fixed-target evidence.

## GC posture

Cyclic GC remains enabled. The object model should avoid large cyclic PHP graphs by keeping high-connectivity authoritative state native-backed. Instrument GC runs, cycles collected, pause duration, roots, PHP memory, native memory, and available allocation data before changing collection policy.

## Concurrency and shutdown

Every cross-thread/runtime path specifies ownership, bounded capacity, backpressure, cancellation, stale-message handling, and shutdown order. Network-shard ownership and gameplay-runtime ownership are independent. Moving a player between gameplay owners must not migrate RakNet ACK/retransmission state.

## Validation doctrine

Implemented, verified, and done are separate states. A generated test is not a passing test. A historical green run does not validate a newer revision. Each bounded child records exact revision evidence before closure.
