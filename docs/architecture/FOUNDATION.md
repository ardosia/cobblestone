# Cobblestone foundation architecture

## Scope

Cobblestone is fixed initially to Minecraft Windows 10 Edition Beta / MCPE 0.15.10, game protocol 84, and RakNet protocol 8. PHP 8.5 ZTS is the controlled high-level runtime. Rust/native code supplies mechanisms and measured hot paths.

The governing split is:

> PHP owns gameplay semantics and the developer/plugin experience. Rust/native extensions own mechanisms, concurrency infrastructure, networking, native representations, and measured hot paths.

This excludes both a Rust server with PHP bolted on as a guest and a traditional single-threaded PHP server that exposes thread primitives to plugins.

## Fixed-target specification

Cross-language facts that define the one supported compatibility target live under root `spec/`. Target identity/protocol numbers, chunk shape, the public block identity/asset catalog plus executable-only End Portal identity, and registered biome IDs/default colors are authored there once. `cargo xtask generate` emits the flat `cobblestone-target` Rust API and the corresponding committed PHP target/catalog sources; `cargo xtask generate --check` is part of repository validation and fails on drift. Generated module paths are implementation details rather than consumer API.

Algorithm-local compatibility values such as RNG constants, structure coordinates, terrain thresholds, and population quirks remain beside the algorithms that give them meaning. Cobblestone policy such as worker limits, queue sizes, view policy, and tick configuration likewise does not belong in the target specification.

## Ownership model

Mutable authoritative objects have one owning PHP runtime. Long-lived identities such as players, entities, worlds, sessions, and selected chunk state use native-backed generational handles. A handle provides identity only. Native ownership metadata determines where mutation is legal.

Wrong-owner behavior is semantic: either route a command to the owner or reject the operation. It must never become cross-runtime arbitrary Zend object access.

Immutable/native shared values such as packet buffers, snapshots, chunk snapshots, encoded blobs, and lookup/catalog data may cross concurrency boundaries without transferring mutable authority.

## Native module boundaries

Initial module families are:

- `cobblestone-target`: generated flat Rust API for shared fixed-target identities/layout sourced from root `spec/`; it contains no gameplay algorithm or server policy.
- `cobblestone-runtime`: runtime identity, generational handles, ownership epochs, and generic region routing. Concrete session/storage concurrency remains owned by those subsystems.
- `cobblestone-world`: authoritative native chunk/world state, snapshots, revisions, residency, patches, and the bounded world change journal.
- `cobblestone-raknet`: fixed-target RakNet state machines and network-shard orchestration. It does not know about Player, World, plugins, or gameplay regions.
- `cobblestone-wire`: fixed-target binary codec, Batch/compression, packet primitives, chunk projection, movement decoding, and NBT. Wire byte ownership uses `bytes::Bytes` and does not depend on runtime worker/handle machinery.
- `cobblestone-session`: stable gameplay-session identity and lifecycle above transport/codec; it hides RakNet connection objects and Batch envelopes from the owning runtime while preserving bounded backpressure and malformed-input behavior.
- `cobblestone-storage`: custom world metadata/region/chunk persistence. The v1 record/region durability core is implemented; async save/load orchestration, metadata publication, and compaction remain isolated here rather than entering `core`.

Sibling modules must not reach into each other's private Rust structs or depend on unstable struct layouts.

## Repository module layout

The production PHP front controller lives at `app/server.php`; PHP product code lives under root `src/`, fixed-target source data under root `spec/`, Rust/native mechanism crates under root `native/`, and repository orchestration under `dev/xtask`. Each Rust crate keeps Cargo-standard local `src/` and test directories.

PHP code lives in one root Composer package under `src/`, organized by semantic namespace. `Cobblestone\\` maps directly to `src/`; domains such as `Session`, `World`, `Command`, and `Plugin` are namespace/filesystem boundaries rather than separately versioned packages.

The root Composer package owns the PHP dependency graph directly. Internal boundaries are enforced by namespace direction, tests, and repository validation rather than local path-package manifests. Command registration/dispatch lives under `src/Command`, owner-runtime event dispatch under `src/Event`, and monotonic tick pacing under `src/Tick`.

Application startup has one configuration boundary. `ApplicationConfig` parses process environment into typed `ServerConfig`, `WorldConfig`, and `StorageConfig`; `Application` performs concrete world/server composition; `app/server.php` only loads the autoloader, obtains that configuration, runs the application, and reports startup failure. Native adapters retain defensive validation at the FFI boundary but do not own application defaults. Repository setup/check/test/build/serve orchestration is implemented by Rust `xtask`; Composer scripts are thin aliases and do not duplicate the workflow.

The PHP world graph is deliberately one-way. `src/World` owns the semantic model and its internal collaborators. `Generator` and `WorldEdit` remain explicit contracts because generation is a real extension point and world editing is the public semantic callback surface. Resident chunk indexing (`MainChunkSource`), mutation coordination (`MutationCoordinator`), region mapping (`RegionMap`), and lighting access are concrete internal mechanisms because Cobblestone has exactly one implementation of each and no plugin-facing substitution requirement. `WorldFactory` remains the composition root for the default graph. Player, entity, block, and inventory remain deferred until their work begins.

The repository-level `modules/` directory and Composer package graph have no role in Zend extension registration. Native PHP functions are registered only by the Rust `ext-php-rs` extension under `native/extension`, and exact exported names are verified independently. Composer/PSR-4 reorganization therefore must not alter the native function ABI.

Rust crate identities remain stable even when repository paths change. Cross-crate public APIs are preserved during source cleanup; internal files are split only along real ownership/dependency seams.

The initial source cleanup applies that rule concretely:
- `cobblestone-runtime` separates generational handle/arena storage and worker public types from pool machinery;
- `cobblestone-world` owns world/chunk state without pulling worker or Zend concerns into that domain;
- `cobblestone-wire` separates packet/NBT data models from wire encode/decode implementation;
- `cobblestone-session` separates session identity, packet, delivery, error, listener/live-session, wire flattening, and host runner concerns;
- `cobblestone-raknet` separates the backend command/state surface from the RakNet event-loop runner;
- `cobblestone-extension` owns the panic/error boundary, runtime identity, diagnostics, session bridge, and fixed-target Zend integration while preserving the `cobblestone_core_php` PHP module/library name.

These are internal source boundaries only. Public crate names, fixed-target behavior, and native PHP function names remain stable.


## C001 — engineering bootstrap

C001 creates the durable project identity, fixed-target requirements, S3 parent program, concrete C001-C003 child changes, architecture/provenance docs, repository instructions, and validation/CI entrypoints. It does not claim any native implementation exists.

C001 is complete only after the resulting remote revision passes the repository metadata validation and repository state is re-read.

## C002 — PHP 8.5 ZTS native-runtime proof

C002 is a sequence of bounded proofs, not one large extension dump.

### C002 primitive slice

The first implementation slice proved the mechanisms that now live in `cobblestone-runtime`, containing:

- `RuntimeId` as an explicit nonzero runtime identity type;
- a type-safe generational handle/arena with stale-generation rejection and slot retirement on generation exhaustion;
- immutable reference-counted native buffers with explicit copy-in and cheap clone semantics.

This slice deliberately contains no Zend calls. It proves ownership primitives independently before FFI is introduced.

### C002 extension boundary slice

The early extension proof established PHP/Zend ownership, thread-affinity, panic containment, safe
error conversion, runtime identity, and stale-handle behavior. Production retains the ABI/runtime
identity contract and internal panic boundary; synthetic probe, deliberate-panic, buffer-copy, and
integer-doubling async exports used only by that proof have been removed.

No third-party PHP threading substrate is part of the public Cobblestone API.

### C002 worker/completion slice

The early bounded-worker/Fiber-completion experiment did not acquire a production consumer. The
generic `cobblestone-runtime` worker/completion API and PHP native-await bridge were therefore
removed. Concrete session and storage concurrency is owned by those subsystems instead.

### C002 measurement

The remaining runtime benchmark covers the production generational-handle lookup mechanism.
Subsystem-specific queue/backpressure and wire-byte behavior are measured and tested at their
actual owners rather than through a synthetic generic worker/buffer layer.

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

C004 promotes the owner-plus-epoch behavior proven by C003 into the reusable mechanism now owned by `cobblestone-runtime`.

`OwnedArena<T>` layers exactly-one-owner metadata over the type-safe generational `Arena`. `OwnedHandle<T>` remains identity only. Mutable/shared authoritative access and reclamation require both the current `RuntimeId` and current `OwnershipEpoch`. Wrong-owner, stale-epoch, stale-handle, and epoch-exhaustion failures are typed and safe.

A successful ownership transfer advances the epoch before the new owner may mutate. Delayed work carrying the previous epoch therefore fails after transfer. If the epoch cannot advance, transfer fails without partially changing owner or value. Removal still delegates slot invalidation to the generational arena, so reused slots reject every old handle.

Semantic routing remains above `cobblestone-runtime`: higher layers may inspect current metadata and route an operation to the owner when API semantics permit, but runtime primitives never silently perform cross-runtime mutation. The C003 ownership torture now wraps the production arena so the stress oracle and production mechanism do not diverge.

Immutable value clones do not transfer mutable authority for the authoritative object that produced them; ownership checks remain attached to the authoritative handle/value.

## C005 — fixed-target RakNet transport

C005 introduces `cobblestone-raknet` as the transport boundary. The implementation pins the exact Ardosia RakNet consumer revision recorded in `docs/provenance/ARDOSIA_REUSE.md` instead of following a moving transport branch.

### Transport boundary

`cobblestone-raknet` owns:

- UDP/RakNet listener and connection lifecycle;
- RakNet reliability selection;
- bounded backend command delivery;
- bounded per-connection inbound payload delivery;
- transport-level backpressure and disconnect behavior;
- clean transport shutdown.

It does not own game protocol 84, packet codecs, batch/compression, NBT, players, worlds, gameplay sessions, plugins, or gameplay ownership.

### Fixed compatibility profile

The initial public configuration is deliberately narrow. `RaknetConfig::new` fixes the listener to the target RakNet protocol and disables the newer handshake-cookie path so the transport matches the accepted MCPE 0.15.10 target. The server advertisement is treated as an opaque transport string supplied by the layer above.

A generic protocol list or modern-Bedrock compatibility switch is not exposed. Expanding the transport target requires an accepted change.

### Bounded facade

Backend commands and per-peer inbound payloads use bounded queues. A peer that exhausts its inbound capacity is closed through an explicit backpressure terminal state instead of creating an unbounded application backlog. Transport ownership remains independent from gameplay-runtime ownership.

### Transport verification

C005 fixtures require raw protocol-8 Request1 acceptance, incompatible protocol rejection, completed protocol-8 connection establishment, bidirectional reliable-ordered payload flow, and reassembly of a payload large enough to exercise RakNet fragmentation.

These fixtures validate RakNet mechanics only. Game-wire compatibility belongs to C006 and later end-to-end fixed-target evidence.

## C006 — protocol-84 wire codec

C006 introduces `cobblestone-wire` above the RakNet transport boundary. It is fixed to MCPE 0.15.10 game protocol 84 and derives compatibility-sensitive wire facts from the supplied fixed-target artifacts plus the pinned matching historical source recorded in `docs/provenance/WIRE.md`.

The codec owns the `0xfe` connected game marker, one-byte protocol-84 packet IDs, fixed-endian packet primitives, Login and Batch zlib framing, the initial login/session bootstrap packet subset, and the little-endian NBT dialect required by protocol-84 network data. It consumes and produces immutable native byte buffers and does not own RakNet reliability, sessions, players, worlds, plugins, authentication policy, or gameplay semantics.

Login preserves the fixed game protocol 84 and the observed 2 MiB decompressed-login cap. Batch contents are a zlib stream of repeated big-endian 32-bit packet length plus raw packet bytes. Operational batch/frame limits remain explicit caller policy rather than invented modern-Bedrock constants.

Network NBT is the historical named-root little-endian mode: little-endian fixed-width numeric values, little-endian 16-bit name/string lengths, and little-endian 32-bit list/array counts. Decoding is bounded by total bytes, recursion depth, collection length, and string bytes. Java big-endian NBT and modern Bedrock network-varint NBT are not silently accepted.

The initial typed session subset covers Login, PlayStatus, Disconnect, Batch, SetTime, StartGame, SetSpawnPosition, AdventureSettings, and SetDifficulty. Authentication/JWT verification, PlayerList, chunks, inventory, and broader gameplay packet semantics remain separate follow-up surfaces.

## C007 — single-runtime server/session foundation

C007 starts from the proven real-client boundary rather than rebuilding transport inside PHP. The production `cobblestone-session` layer assigns stable process-local session IDs, accepts fixed-target game payloads through `cobblestone-raknet`, removes the outer game marker, flattens bounded Batch/compression envelopes through `cobblestone-wire`, validates outbound frames before transport submission, closes malformed peers, and preserves typed transport backpressure/disconnect errors.

The session layer is still internal wire/session infrastructure. A dedicated `SessionHost` thread owns the async mechanism and communicates with the PHP owner only through bounded native event/command queues. The PHP-side `Cobblestone\\Native\\Session` adapter tracks local lifecycle state and converts native events into server-internal PHP values. Owner-runtime identity is enforced once at the actual native operation boundary by `cobblestone-extension`; PHP does not perform redundant native owner/running probes before every poll/send/disconnect call. PHP remains the owner of gameplay semantics, lifecycle callbacks, events, commands, plugin loading, scheduler state, and Fiber resumption. RakNet connection objects, transport queues, native worker primitives, and protocol packet structs do not become ordinary plugin APIs.

The ordinary PHP server surfaces are deliberately synchronous and owner-local. Event dispatch, command compilation, and plugin hosting live behind `Event\Internal\Dispatcher`, `Command\Internal\CommandTree`, and `Plugin\Internal\Plugins`; `Scheduler` remains the owner-runtime task mechanism. Plugins receive an owned `PluginScope`; event subscriptions, command bindings, tasks, and cleanup registered through that scope are released deterministically, including rollback after failed enable. `Server` construction is side-effect free until explicit `start()`, then translates native connect/disconnect state into semantic PHP events, keeps raw wire packets on an internal handler, and applies a finite native-event budget per tick. Scheduled callbacks and sleeping Fibers are indexed by stable due-time binary min-heaps; the heap and sleep marker live under `Task\\Internal`, so a tick visits due work rather than the entire live set. There is no generic PHP native-await API: session and storage completion behavior remains inside their concrete native mechanisms. The fixed-target real-client bootstrap is orchestrated by an internal PHP state machine while Login validation, packet encoding, and compression remain native wire mechanisms; obsolete compatibility-only synthetic bootstrap exports have been removed under ABI version 2.

## World substrate and Flat generation

`Cobblestone\\World\\World` is the owner-runtime semantic root. `MainChunkSource` is the concrete resident PHP index and generator boundary; mutation coordination, region mapping, and staged lighting are internal concrete collaborators rather than ceremonial interfaces. Flat generation remains behind the `Generator` contract because generation is intentionally replaceable. Native handles, locks, regions, and transport objects remain outside ordinary gameplay APIs.

The chunk model is fixed to the evidenced 0.15.10 structure: 16×16 horizontal chunks, eight 16-block vertical sections, Y 0..127, legacy block id+data state, per-column biome identity, heightmap, separate sky/block light, and sparse 16-bit block extra data. Negative world coordinates use Euclidean/floor chunk mapping. Hot paths use scalar legacy state ids; `BlockState` remains an ergonomic wrapper, not the storage currency.

When the native extension is available, `WorldFactory` creates one region-sharded native `WorldStore` and generated `Chunk` objects remain owner-runtime PHP semantic facades. Backend mechanics sit behind the internal `ChunkState` boundary, which has exactly two active implementations: native WorldStore-backed state and the parity-tested PHP fallback. Scalar reads/writes, terrain/light revisions, snapshots, patch commits, lifecycle flags, persistence dirtiness, pin mirroring, and eviction policy all route through that boundary instead of branching throughout the world model.

The PHP/native boundary is deliberately coarse. Flat generation uses bulk layer/light fills. Immutable `ChunkSnapshot` projections carry complete semantic chunk state in one read. Staged world mutations commit one revision-checked backend patch per changed chunk and automatically switch large native-backed read sets to one snapshot instead of continuing per-cell FFI reads. Fixed-target light propagation remains PHP gameplay semantics, but reads one immutable snapshot per touched chunk, stages scalar light levels locally, validates the exact terrain/light revisions it read, and commits one backend light patch per changed chunk; the native implementation performs that commit as one atomic FFI patch.

Protocol-84 initial chunk streaming no longer round-trips chunk planes through PHP: the extension reads immutable native snapshots directly, caches encoded FullChunkData by terrain/light revision, builds the compressed Batch, and submits it through the session host. Persistent initial views use a pre-encoded native load batch; `Session\\Internal\\Bootstrap` parks the session while Rust workers load/import chunks, adopts loaded native residents without rewriting terrain, generates only durable misses, and spawns only when the whole requested view is resident. Live world changes follow the same boundary. Successful native patches append to a bounded sequenced change journal; one owner-runtime flush call per server tick filters/coalesces changes for spawned viewers, uses UpdateBlock for bounded block-state-only edits, falls back to FullChunkData for changes that need a complete semantic projection, and preserves viewer cursors across native queue backpressure. PHP never serializes a changed-block list for network delivery.

Generator ids retain the fixed StartGame vocabulary (old=0, infinite=1, flat=2). Flat uses the historical version-2 `2;7,2x3,2;1;` preset; Infinite now exposes the exact recovered 0.15.10 Overworld pipeline through native composition, while Old remains deferred. Persistence v1 is structurally complete: durable world metadata, region storage, bounded async save/load, resident/pinned lifecycle, deferred persistent initial-view loading, production Flat and Infinite world-directory composition, explicit server-stop persistence flush, generator-owned Infinite structure-state sidecar persistence, the Fiber-friendly pinned gameplay acquisition path, bounded clean/unpinned eviction, region churn accounting, crash-safe region rewriting, bounded region-sharded compaction scheduling, and the conservative 64 MiB + 50% production compaction gate are implemented. Further storage work is operational tuning or format/tooling evolution rather than a blocker for gameplay.

See `docs/provenance/WORLD015.md` for the fixed-target evidence boundary, `docs/architecture/WORLD_SYNC.md` for live synchronization, and `docs/architecture/WORLD_STORAGE.md` for the persistence format contract.

## Runtime hardening, mutations, and execution regions

The application logging boundary is PSR-3. The default implementation uses Monolog and a Spring Boot-inspired console layout containing millisecond timestamp, level, PID, application name, execution label, logger name, message, and structured key/value context. Cobblestone does not print a startup banner. Plugins receive scoped `LoggerInterface` instances and are not coupled to Monolog.

The application delegates pacing to a monotonic `TickLoop` instead of owning a raw infinite loop. The loop targets the configured tick rate, reports sustained lateness as both milliseconds and ticks behind, throttles warnings, and rebases after excessive backlog rather than spinning through obsolete deadlines. Task scheduling is separately due-indexed: dormant callbacks and sleeping Fibers contribute heap storage but no linear per-tick scan; cancellation uses lazy invalidation with bounded periodic heap compaction.

`Server` owns an explicit Starting/Running/Stopping/Stopped lifecycle. Stop requests end the loop after the current tick. SIGINT/SIGTERM are handled where pcntl exists, and a PHP shutdown hook provides a final best-effort stop. Shutdown continues through stopping-event dispatch, native-session shutdown, plugin disable, scheduler shutdown, and explicit persistent-world flushing even if an earlier phase fails.

Gameplay world changes use `World::edit()`. `WorldEdit` is the semantic callback surface; `StagedWorldMutation` provides read-your-writes semantics and may be replayed before commit. The concrete `MutationCoordinator` discovers the touched chunk set, discards/replays when that set expands, prepares every chunk against a base revision, rejects stale revisions, filters net-no-op/reverted edits, then commits changed chunks with one revision advance each. `World` owns that concrete internal coordinator directly while generation remains direct initialization.

Execution regions are internal ownership/scheduling territories, not fixed-target Minecraft world/storage semantics. PHP maps chunks deterministically to internal region identities. Rust `cobblestone-runtime::RegionDirectory` maps those identities to exactly one `RuntimeId` plus `OwnershipEpoch`, rejecting wrong-owner and stale-epoch transfers. Normal plugin APIs expose neither regions nor runtime/thread ceremony.

The production gameplay path remains single-owner PHP today. This foundation deliberately does not claim that multi-runtime region scheduling is active yet.

See `docs/provenance/RUNTIME_FOUNDATION.md`.

## GC posture

Cyclic GC remains enabled. The object model should avoid large cyclic PHP graphs by keeping high-connectivity authoritative state native-backed. Instrument GC runs, cycles collected, pause duration, roots, PHP memory, native memory, and available allocation data before changing collection policy.

## Concurrency and shutdown

Every cross-thread/runtime path specifies ownership, bounded capacity, backpressure, cancellation, stale-message handling, and shutdown order. Network-shard ownership and gameplay-runtime ownership are independent. Moving a player between gameplay owners must not migrate RakNet ACK/retransmission state.

## Validation doctrine

Implemented, verified, and done are separate states. A generated test is not a passing test. A historical green run does not validate a newer revision. Each bounded child records exact revision evidence before closure.