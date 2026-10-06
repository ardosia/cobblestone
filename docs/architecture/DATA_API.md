# Configuration, catalogs, persistence, diagnostics, and data design

## Status

This document defines cross-cutting data-oriented subsystem design for both current and future Cobblestone work.

It covers:

- configuration;
- fixed-target catalogs/registries;
- world/player/entity/plugin persistence boundaries;
- logging;
- metrics/diagnostics;
- native ABI/capability reporting;
- administrative state inspection.

World region/chunk durability details remain in WORLD_STORAGE.md.

## Configuration

Configuration is typed data, not a mutable service graph.

Prefer readonly values loaded once near the composition root:

~~~php
$config = new ServerConfig(
    bind: '0.0.0.0:19132',
    name: 'Cobblestone',
    maxConnections: 200,
    initialChunkRadius: 3,
    tickRate: 20,
);
~~~

Environment variables, files, and CLI arguments are input sources.

They should be parsed/validated into typed configuration before normal subsystem execution.

Subsystems should not repeatedly call getenv() during gameplay.

## Configuration sources

Source precedence must be deterministic.

A future loader may combine:

1. built-in defaults;
2. configuration file;
3. environment overrides;
4. explicit CLI overrides.

The application source loader produces one `ApplicationConfig`, which owns the typed `ServerConfig`, `WorldConfig`, and `StorageConfig` values for that process.

Do not let each subsystem invent its own precedence rules.

## Secrets

If future authentication/integration work requires secrets, secret values should be separated from normal printable configuration.

Diagnostics must redact them.

Cobblestone should not create a generic secret manager until a real external-secret requirement exists.

## Runtime configuration changes

Only values with real runtime semantics should be mutable after start.

Changing max players, view distance, logging level, or similar values may eventually have dedicated operations.

Do not mutate a shared ServerConfig object in place and expect every subsystem to notice.

Runtime change is a semantic command with explicit validation/publication.

## Catalogs

Cobblestone has fixed-target game data such as:

- block types/states;
- item types;
- entity types;
- biome identifiers;
- effects;
- attributes;
- protocol packet identifiers.

These are catalogs, not arbitrary mutable registries.

The fixed protocol/source target is authoritative.

## Catalog identity

Catalog entries should expose semantic typed identities.

Examples:

~~~text
BlockType
ItemType
EntityType
BiomeType
EffectType
AttributeType
~~~

Protocol numeric IDs are implementation details attached to catalog entries where wire/storage compatibility needs them.

Ordinary plugin code should prefer semantic identity.

## Closed vs extensible domains

Use enums when the domain is truly closed and ergonomic as an enum. The fixed 0.15.10 block identity domain follows this rule directly: `BlockType` is a backed enum, with the legacy wire/storage ID as its backing value, while exact asset vocabulary and static metadata remain attached to those singleton identities.

Use catalog/registry objects when:

- values carry richer metadata;
- lookup by protocol ID/name is required;
- extensibility is meaningful;
- the set is too large or generated.

Do not force every catalog into PHP enums.

## Registry naming

Use Registry only for a real map of registered values/behaviors.

Do not append Registry to ordinary services for architectural decoration.

Examples of legitimate registries may include:

- compiled command roots;
- plugin-defined recipes;
- plugin-defined permissions;
- block/item behavior tables.

## Fixed-target extension limits

Protocol-84 clients cannot automatically understand arbitrary new block/item/entity wire IDs.

A plugin may register server-side behavior or aliases where supported, but the API must not imply that custom protocol identities are available when the client cannot represent them.

Server-semantic extensibility and protocol extensibility are different capabilities.

## Catalog generation

Large fixed-target tables should be generated or loaded from pinned source/protocol data where practical.

Hand-maintained duplicated numeric constants should be minimized.

Generated data should have provenance and deterministic output.

## World persistence

World terrain/chunk persistence remains owned by the native storage subsystem.

Gameplay mutates World.

Dirty tracking and storage workers persist snapshots asynchronously.

Plugins do not write native region records directly.

WORLD_STORAGE.md remains the durability authority.

## Player persistence

Future persistent Player state should be separated into a profile/data snapshot independent from a live Player object.

Conceptually:

~~~php
final readonly class PlayerData
{
    // identity, position, inventory, game mode, etc.
}
~~~

The exact fields follow implemented gameplay/source parity.

Loading creates owned gameplay state from an immutable data snapshot.

Saving serializes an immutable snapshot from the owner runtime for background persistence.

## Player repository

A future persistence boundary may resemble:

~~~php
interface PlayerStore
{
    public function load(PlayerKey $player): NativeJob|PlayerLoad;
    public function save(PlayerData $data): NativeJob|PlayerSave;
}
~~~

The final async type should reuse Cobblestone's task/native completion model rather than invent a competing promise framework.

The point is to keep live Player objects out of worker/storage threads.

## Entity persistence

Not every Entity must be persistent.

Persistent entity state should snapshot only entities whose lifecycle requires durability.

Chunk/world unload should not accidentally persist temporary/projectile entities unless fixed-target semantics require it.

Entity persistence identity must be explicit enough to avoid duplicate respawn after partial save/load failures.

## Inventory persistence

Inventory data is saved as immutable ItemStack/slot data.

Protocol window state is not persistent.

An open ContainerSession is runtime state and is closed on disconnect/shutdown.

## Plugin data

Plugins need a scoped durable-data surface eventually.

The core should not invent a universal ORM/database abstraction.

A minimal plugin-owned store may provide atomic versioned document/blob operations under a plugin namespace.

For example, conceptually:

~~~php
$data = $plugin->data();

$value = $data->read('config/state');
$data->write('config/state', $document);
~~~

The exact format/API should be decided with the first real plugin persistence use case.

Required properties are clearer than syntax:

- plugin namespace isolation;
- safe/atomic publication;
- version/migration support;
- background I/O where needed;
- no arbitrary access to world-native storage files.

## Data migrations

Persistent formats need explicit versions.

Migrations run at controlled open/load boundaries.

Do not silently reinterpret incompatible bytes as current state.

World storage, player data, and plugin data may have different version timelines.

A shared migration framework is not required unless common behavior emerges.

## Save ownership

Each subsystem decides when state becomes dirty.

Persistence infrastructure decides how dirty snapshots reach durable storage.

This separation avoids exposing file-write timing as gameplay semantics.

A successful gameplay mutation does not mean fsync completed.

Shutdown durability guarantees must be documented separately.

## Atomic publication

Durable metadata/index/document updates should use an atomic publication strategy appropriate to the format.

Avoid in-place partial writes that can make the only authoritative copy unreadable after process failure.

WORLD_STORAGE.md defines the concrete world-region rules.

Future player/plugin formats must define equivalent crash semantics before being called durable.

## Background I/O

Disk I/O must not block the PHP owner runtime during normal gameplay.

Owner runtime work should be:

- create immutable snapshot/request;
- submit bounded native/background job;
- consume completion;
- update semantic state.

Backpressure is explicit when queues fill.

## Persistence errors

Persistence failures must be typed and observable.

The subsystem decides whether a failure:

- rejects one operation;
- keeps state dirty for retry;
- blocks unload/shutdown completion;
- marks a world/profile read-only/unavailable;
- escalates to server stop.

Logging an error and pretending data is safely persisted is not acceptable.

## Logging

Cobblestone uses PSR-3-compatible logging as the application logging contract.

The current Monolog-backed implementation can remain the default.

The console renderer is presentation only.

Structured context should remain available underneath.

## Logger ownership

Server has a root logger.

Subsystems and plugins may derive contextual loggers.

Conceptually:

~~~php
$logger = $plugin->logger();

$logger->info(
    'Generated arena',
    ['world' => $world->id(), 'chunks' => $count],
);
~~~

Do not use global static logger access as the normal path.

## Log levels

Hot paths should not eagerly build expensive debug context when the level is disabled.

Repeated per-packet/per-block logging is diagnostic mode behavior, not default production behavior.

Rate-limit or aggregate repeated warnings where one root problem can generate thousands of identical lines.

## Diagnostics

Diagnostics answer questions about current runtime state without becoming a second management API.

Examples include:

~~~text
tick timing
task counts
native queue depth
session counts
world resident/dirty chunks
storage queue depth
cache hit/miss counters
network backpressure/disconnect counts
plugin/task failures
~~~

Values should be cheap snapshots/counters.

Diagnostics must not require scanning every entity/chunk on every read unless the operation is explicitly expensive.

## Metrics

Metrics should be numeric and aggregatable.

Useful primitives include:

- counter;
- gauge;
- histogram/timing.

The implementation may initially expose diagnostics without a network exporter.

Prometheus/OpenTelemetry integration is an adapter concern, not the semantic core.

Do not hardwire one external metrics vendor into gameplay packages.

## Tracing

Native Rust code already has tracing-compatible infrastructure.

Cross-boundary operations may carry compact correlation/session/task IDs for diagnostics.

Do not propagate full PHP objects or huge structured payloads purely for tracing.

Tracing must remain optional enough not to dominate hot-path cost.

## Administrative snapshots

A future admin/status API may expose immutable snapshots:

~~~php
$status = $server->status();

$status->players();
$status->tick();
$status->memory();
$status->sessions();
~~~

This is read-only observation.

Administrative mutation should still call semantic operations such as stop(), kick(), save(), or plugin commands.

Do not create a generic setProperty(name, mixed) control plane.

## Native ABI

The PHP extension exposes one stable ABI version through `cobblestone_core_abi()` rather than requiring PHP to manually check a growing list of function names.

The session Runtime adapter validates that ABI once at startup. ABI version 1 is the current contract.

Capability bits/metadata may be added later if optional native features need staged rollout; they are not required while the extension is one fixed feature set.

## ABI compatibility

ABI identity should include enough information to reject incompatible PHP/native module combinations clearly.

Do not treat presence of one function as proof that every required export has compatible behavior.

Capabilities may support staged rollouts where optional native features are present on some builds.

## Native result mapping

Internal native result codes should map centrally to PHP enums/exceptions/value results.

Do not scatter comparisons such as:

~~~php
if ($result === 2) {
    // ...
}
~~~

through gameplay packages.

The extension adapter is responsible for translating wire/FFI codes into stable semantic categories. Prepared-view send results already cross into PHP as `ViewSendResult` rather than raw 0/1/2 status integers.

## Clock

Time-sensitive runtime code should depend on the server Clock abstraction rather than reading wall clock ad hoc.

Tick scheduling uses monotonic time.

Wall-clock timestamps are for logs/persistence metadata where appropriate.

Tests should be able to supply deterministic/fake clocks to lifecycle/timing components.

## Randomness

Gameplay randomness that affects deterministic world/game behavior should use explicit sources/seeds.

Do not call random_int()/mt_rand() across unrelated systems with hidden global state when reproducibility matters.

Cryptographic randomness is a separate concern for identifiers/tokens if future systems require it.

## IDs

Stable IDs should be typed according to domain.

Do not reuse one global integer namespace for:

~~~text
SessionId
PlayerId
EntityId
WorldId
TaskId
NativeHandle
~~~

Typed wrappers/enums/native handle classes prevent cross-domain mistakes.

## Serialization

Serialization formats belong to the subsystem that owns the data.

Do not create one generic PHP serializer for:

- protocol packets;
- world chunks;
- player data;
- plugin config;
- logs.

They have different compatibility/durability/security constraints.

## Input validation

Untrusted external data includes:

- network packets;
- player names/text;
- plugin configuration;
- persistent files after crashes/manual edits;
- plugin-provided registrations.

Validate at subsystem boundaries.

Once validated/compiled, internal hot paths should operate on typed structures rather than repeatedly revalidating the same representation.

## Observability and privacy

Logs/diagnostics should avoid unnecessarily exposing:

- authentication tokens;
- private external credentials;
- full raw network payloads;
- unbounded NBT/document contents.

Debug tooling may provide explicit bounded dumps when needed.

Default production diagnostics remain compact.

## Testing

Data-oriented tests should cover:

- configuration precedence/validation;
- catalog lookup/protocol parity;
- persistence version rejection/migration;
- async save/load backpressure;
- crash/atomic publication behavior;
- plugin namespace isolation;
- ABI mismatch reporting;
- diagnostic counter correctness;
- disabled-log hot-path behavior;
- deterministic clock/random test fixtures.

## Design summary

The data layer should converge on:

> Parse configuration once. Treat fixed game data as typed catalogs. Snapshot live gameplay state before background persistence. Keep file formats owned by their subsystem. Use structured logging and cheap diagnostics. Validate the native ABI once instead of probing dozens of symbols. Do not turn storage or observability into gameplay managers.
