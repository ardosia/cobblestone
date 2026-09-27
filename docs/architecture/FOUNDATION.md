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

- `cobblestone-core`: runtime identity, generational handles, bounded workers, completion/future plumbing, Fiber wake integration, cancellation, immutable buffers, telemetry, panic/error boundaries, region routing, and the current region-sharded native `WorldStore`.
- `cobblestone-network`: protocol-8 RakNet state machines and network-shard orchestration. It does not know about Player, World, plugins, or gameplay regions.
- `cobblestone-codec`: protocol-84 binary codec, batch/compression, packet primitives, NBT where appropriate, and native-buffer integration.
- `cobblestone-session`: stable gameplay-session identity and lifecycle above transport/codec; it hides RakNet connection objects and Batch envelopes from the owning runtime while preserving bounded backpressure and malformed-input behavior.
- A separate `cobblestone-world` crate is deferred until the native world mechanism needs an independently versioned boundary; splitting the already-working store merely for taxonomy is not a goal.
- `cobblestone-storage`: custom world metadata/region/chunk persistence. The v1 record/region durability core is implemented; async save/load orchestration, metadata publication, and compaction remain isolated here rather than entering `core`.

Sibling modules must not reach into each other's private Rust structs or depend on unstable struct layouts.

## Repository module layout

All product code lives under `modules/`. Rust mechanism crates live in `modules/rust/`; each keeps Cargo-standard local `src/` and test directories.

PHP code is a flat local Composer-package workspace under `modules/php/`. Direct child package directories and Composer package identities are lowercase (for example `modules/php/session` and `ardosia/cobblestone-session`), while PHP namespace/class identity remains PascalCase (for example `Cobblestone\\Session\\JoinFlow`). Each package root is its PSR-4 source root; package-local `src/` wrappers are deliberately omitted.

The root Composer application consumes `modules/php/*` as path repositories and composes the running application through the server package. Package manifests declare their actual sibling dependencies, so boundaries are dependency-enforced rather than a decorative filesystem split. Small command and event primitives that share the same owner-runtime consumers live together in `cobblestone-kernel`; monotonic tick pacing lives with `cobblestone-server`, its sole lifecycle owner.

The PHP world graph is deliberately one-way. `cobblestone-world` owns the semantic model plus the collaboration contracts/value types that `World` exposes: `Generator`/`GeneratorType`, `ChunkSource`, `RegionMapInterface`/`RegionId`, `WorldMutation`, `MutationCoordinatorInterface`, and `MutationResult`. Concrete `cobblestone-world-generation`, `cobblestone-world-light`, `cobblestone-world-mutation`, and `cobblestone-world-region` packages each depend only on `cobblestone-world`. `World` accepts its already-built collaborators and never constructs those implementations itself. `cobblestone-server` depends on the base plus all four world implementation packages and is the composition root that wires the default graph. This keeps the Composer dependency graph acyclic without hiding back-edges inside namespace layout. Player, entity, block, and inventory remain deferred until their work begins.

The repository-level `modules/` directory and Composer package graph have no role in Zend extension registration. Native PHP functions are registered only by the Rust `ext-php-rs` extension under `modules/rust/php-extension`, and exact exported names are verified independently. Composer/PSR-4 reorganization therefore must not alter the native function ABI.

Rust crate identities remain stable even when repository paths change. Cross-crate public APIs are preserved during source cleanup; internal files are split only along real ownership/dependency seams.

The initial source cleanup applies that rule concretely:
- `cobblestone-core` separates generational handle/arena storage and worker public types from pool machinery;
- `cobblestone-codec` separates packet/NBT data models from wire encode/decode implementation;
- `cobblestone-session` separates session identity, packet, delivery, error, listener/live-session, wire flattening, and host runner concerns;
- `cobblestone-network` separates the backend command/state surface from the RakNet event-loop runner;
- `cobblestone-core-php` separates panic/error boundary, runtime identity, diagnostics, session bridge, and fixed-target join compatibility machinery.

These are internal source boundaries only. Public crate names, fixed-target behavior, and native PHP function names remain stable.


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

## C007 — single-runtime server/session foundation

C007 starts from the proven real-client boundary rather than rebuilding transport inside PHP. The production `cobblestone-session` layer assigns stable process-local session IDs, accepts protocol-84 payloads through `cobblestone-network`, removes the outer game marker, flattens bounded Batch/compression envelopes through `cobblestone-codec`, validates outbound frames before transport submission, closes malformed peers, and preserves typed transport backpressure/disconnect errors.

The session layer is still internal wire/session infrastructure. A dedicated `SessionHost` thread owns the async mechanism and communicates with the PHP owner only through bounded native event/command queues. The PHP-side `Cobblestone\\Native\\Session\\Runtime` facade tracks local lifecycle state and converts native events into server-internal PHP values. Owner-runtime identity is enforced once at the actual native operation boundary by `cobblestone-core-php`; PHP does not perform redundant native owner/running probes before every poll/send/disconnect call. PHP remains the owner of gameplay semantics, lifecycle callbacks, events, commands, plugin loading, scheduler state, and Fiber resumption. RakNet connection objects, transport queues, native worker primitives, and protocol packet structs do not become ordinary plugin APIs.

The first ordinary PHP server surfaces are deliberately synchronous and owner-local: `EventBus`, `CommandRegistry`, `PluginManager`, and `Scheduler`. Plugins receive only `PluginContext` with those facilities. `Server` translates native connect/disconnect state into semantic PHP events, keeps raw wire packets on an internal handler, applies a finite native-event budget per tick, and disconnects sessions whose raw packets have no installed server handler. Scheduled callbacks and `TickSleep` Fibers are indexed by stable due-time binary min-heaps so a tick visits due work rather than the entire live set. Fiber waits on native completions remain a compact active-wait set and are polled only during the owner-runtime scheduler tick; the current diagnostic worker ABI exposes per-task `ready`/`take` operations and no batch ready-set. The fixed-target real-client bootstrap is now orchestrated by an internal PHP state machine while Login validation, packet encoding, compression, and compatibility-only synthetic probe exports remain native wire mechanisms.

## World substrate and Flat generation

`Cobblestone\\World\\World` is the owner-runtime semantic root. `BlockSource` and `ChunkSource` preserve useful fixed-target access seams without exposing native handles, locks, regions, or transport objects to ordinary gameplay code. `MainChunkSource` is the resident PHP index and generator boundary; the base world package owns the shared generator/mutation/light/region contracts, while Flat generation, fixed-target light propagation, staged mutation, and region mapping remain replaceable satellite packages composed by the server.

The chunk model is fixed to the evidenced 0.15.10 structure: 16×16 horizontal chunks, eight 16-block vertical sections, Y 0..127, legacy block id+data state, per-column biome identity, heightmap, separate sky/block light, and sparse 16-bit block extra data. Negative world coordinates use Euclidean/floor chunk mapping. Hot paths use scalar legacy state ids; `BlockState` remains an ergonomic wrapper, not the storage currency.

When the native extension is available, `WorldFactory` creates one region-sharded native `WorldStore` and generated `Chunk` objects become owner-runtime PHP facades over that store. Native owns the packed terrain/light planes, biome and height data, sparse extra data, terrain/light revisions, immutable snapshots, and atomic patch application. The PHP-backed representation remains a behavioral fallback for environments without the extension and is kept parity-tested against the native path.

The PHP/native boundary is deliberately coarse. Flat generation uses bulk layer/light fills. Immutable `ChunkSnapshot` projections carry complete semantic chunk state in one read. Staged world mutations commit one revision-checked patch per changed chunk and automatically switch large native-backed read sets to one snapshot instead of continuing per-cell FFI reads. Fixed-target light propagation remains PHP gameplay semantics, but reads one immutable snapshot per touched chunk, stages scalar light levels locally, validates the exact terrain/light revisions it read, and commits one native light patch per changed chunk.

Protocol-84 initial chunk streaming no longer round-trips chunk planes through PHP: the extension reads immutable native snapshots directly, caches encoded FullChunkData by terrain/light revision, builds the compressed Batch, and submits it through the session host. Live world changes follow the same boundary. Successful native patches append to a bounded sequenced change journal; one owner-runtime flush call per server tick filters/coalesces changes for spawned viewers, uses UpdateBlock for bounded block-state-only edits, falls back to FullChunkData for changes that need a complete semantic projection, and preserves viewer cursors across native queue backpressure. PHP never serializes a changed-block list for network delivery.

Generator ids retain the fixed StartGame vocabulary (old=0, infinite=1, flat=2), but only Flat is implemented. Its default preset is the historical version-2 `2;7,2x3,2;1;` layout (bedrock, two dirt, grass, biome 1). Decoration, durable persistence, broad block behavior, and non-flat generation remain separate work. The custom persistence format and native resident/pinned lifecycle are now defined; asynchronous storage I/O is the next persistence milestone.

See `docs/provenance/WORLD015.md` for the fixed-target evidence boundary, `docs/architecture/WORLD_SYNC.md` for live synchronization, and `docs/architecture/WORLD_STORAGE.md` for the persistence format contract.

## Runtime hardening, mutations, and execution regions

The application logging boundary is PSR-3. The default implementation uses Monolog and a Spring Boot-inspired console layout containing millisecond timestamp, level, PID, application name, execution label, logger name, message, and structured key/value context. Cobblestone does not print a startup banner. Plugins receive scoped `LoggerInterface` instances and are not coupled to Monolog.

The executable delegates pacing to a monotonic `TickLoop` instead of owning a raw infinite loop. The loop targets the configured tick rate, reports sustained lateness as both milliseconds and ticks behind, throttles warnings, and rebases after excessive backlog rather than spinning through obsolete deadlines. Task scheduling is separately due-indexed: dormant callbacks and sleeping Fibers contribute heap storage but no linear per-tick scan; cancellation uses lazy invalidation with bounded periodic heap compaction.

`Server` owns an explicit Starting/Running/Stopping/Stopped lifecycle. Stop requests end the loop after the current tick. SIGINT/SIGTERM are handled where pcntl exists, and a PHP shutdown hook provides a final best-effort stop. Shutdown continues through stopping-event dispatch, native-session shutdown, plugin disable, and scheduler shutdown even if an earlier phase fails.

Gameplay world changes use `World::mutate()`. The base `WorldMutation` interface is the semantic callback surface; the staged implementation lives in `cobblestone-world-mutation`, provides read-your-writes semantics, and may be replayed before commit. The concrete coordinator discovers the touched chunk set, discards/replays when that set expands, prepares every chunk against a base revision, rejects stale revisions, filters net-no-op/reverted edits, then commits changed chunks with one revision advance each. `World` depends only on `MutationCoordinatorInterface`; ordinary mutation conveniences use that seam while generation remains direct initialization.

Execution regions are internal ownership/scheduling territories, not fixed-target Minecraft world/storage semantics. PHP maps chunks deterministically to internal region identities. Rust `cobblestone-core::RegionDirectory` maps those identities to exactly one `RuntimeId` plus `OwnershipEpoch`, rejecting wrong-owner and stale-epoch transfers. Normal plugin APIs expose neither regions nor runtime/thread ceremony.

The production gameplay path remains single-owner PHP today. This foundation deliberately does not claim that multi-runtime region scheduling is active yet.

See `docs/provenance/RUNTIME_FOUNDATION.md`.

## GC posture

Cyclic GC remains enabled. The object model should avoid large cyclic PHP graphs by keeping high-connectivity authoritative state native-backed. Instrument GC runs, cycles collected, pause duration, roots, PHP memory, native memory, and available allocation data before changing collection policy.

## Concurrency and shutdown

Every cross-thread/runtime path specifies ownership, bounded capacity, backpressure, cancellation, stale-message handling, and shutdown order. Network-shard ownership and gameplay-runtime ownership are independent. Moving a player between gameplay owners must not migrate RakNet ACK/retransmission state.

## Validation doctrine

Implemented, verified, and done are separate states. A generated test is not a passing test. A historical green run does not validate a newer revision. Each bounded child records exact revision evidence before closure.