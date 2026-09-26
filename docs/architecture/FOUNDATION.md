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

The validated C003 outcome is **GO for the process-isolated persistent-runtime topology**. It does not prove same-process or cross-thread embedded Zend safety; adopting such a topology later requires a separate proof.

## C004 — production ownership mechanism

C004 promotes the owner-plus-epoch behavior proven by C003 into reusable `cobblestone-core` mechanism.

`OwnedArena<T>` layers exactly-one-owner metadata over the type-safe generational `Arena`. `OwnedHandle<T>` remains identity only. Mutable/shared authoritative access and reclamation require both the current `RuntimeId` and current `OwnershipEpoch`. Wrong-owner, stale-epoch, stale-handle, and epoch-exhaustion failures are typed and safe.

A successful ownership transfer advances the epoch before the new owner may mutate. Delayed work carrying the previous epoch therefore fails after transfer. If the epoch cannot advance, transfer fails without partially changing owner or value. Removal still delegates slot invalidation to the generational arena, so reused slots reject every old handle.

Semantic routing remains above `cobblestone-core`: higher layers may inspect current metadata and route an operation to the owner when API semantics permit, but core never silently performs cross-runtime mutation. The C003 ownership torture now wraps the production arena so the stress oracle and production mechanism do not diverge.

Immutable native values such as `NativeBuffer` may be cloned/shared across runtimes. Sharing immutable data never transfers mutable authority for the authoritative object that produced it.

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

## C006 — protocol-84 wire codec

C006 introduces `cobblestone-codec` above the RakNet transport boundary. It is fixed to MCPE 0.15.10 game protocol 84 and derives compatibility-sensitive wire facts from the supplied fixed-target artifacts plus the pinned matching historical source recorded in `docs/provenance/PROTOCOL84.md`.

The codec owns the `0xfe` connected game marker, one-byte protocol-84 packet IDs, fixed-endian packet primitives, Login and Batch zlib framing, the initial login/session bootstrap packet subset, and the little-endian NBT dialect required by protocol-84 network data. It consumes and produces immutable native byte buffers and does not own RakNet reliability, sessions, players, worlds, plugins, authentication policy, or gameplay semantics.

Login preserves the fixed game protocol 84 and the observed 2 MiB decompressed-login cap. Batch contents are a zlib stream of repeated big-endian 32-bit packet length plus raw packet bytes. Operational batch/frame limits remain explicit caller policy rather than invented modern-Bedrock constants.

Network NBT is the historical named-root little-endian mode: little-endian fixed-width numeric values, little-endian 16-bit name/string lengths, and little-endian 32-bit list/array counts. Decoding is bounded by total bytes, recursion depth, collection length, and string bytes. Java big-endian NBT and modern Bedrock network-varint NBT are not silently accepted.

The initial typed session subset covers Login, PlayStatus, Disconnect, Batch, SetTime, StartGame, SetSpawnPosition, AdventureSettings, and SetDifficulty. Authentication/JWT verification, PlayerList, chunks, inventory, and broader gameplay packet semantics remain separate follow-up surfaces.

## GC posture

Cyclic GC remains enabled. The object model should avoid large cyclic PHP graphs by keeping high-connectivity authoritative state native-backed. Instrument GC runs, cycles collected, pause duration, roots, PHP memory, native memory, and available allocation data before changing collection policy.

## Concurrency and shutdown

Every cross-thread/runtime path specifies ownership, bounded capacity, backpressure, cancellation, stale-message handling, and shutdown order. Network-shard ownership and gameplay-runtime ownership are independent. Moving a player between gameplay owners must not migrate RakNet ACK/retransmission state.

## Validation doctrine

Implemented, verified, and done are separate states. A generated test is not a passing test. A historical green run does not validate a newer revision. Each bounded child records exact revision evidence before closure.
