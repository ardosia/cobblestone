# Cobblestone subsystem map

## Purpose

This document is the design index for the whole server.

It records:

- subsystems that already exist;
- subsystems that are only partially present;
- gameplay/runtime subsystems that do not exist yet but are expected to be needed;
- the architecture document that owns the public/API direction for each area.

This is a design map, not a requirement to create one Composer package or Rust crate for every row.

Package boundaries are introduced only when implementation creates a real ownership/dependency seam.

## Global rules

All subsystem work is constrained by:

- FOUNDATION.md — PHP/Rust ownership model and repository architecture;
- API_STYLE.md — cross-cutting PHP API style;
- COMMANDS.md — command-tree design;
- RUNTIME_API.md — server lifecycle, events, tasks, plugins, logging/runtime ownership;
- TRANSPORT_API.md — network, codec, session, login/bootstrap boundaries;
- WORLD_API.md — world/chunk/block/generation/light/storage semantic API;
- GAMEPLAY_API.md — player/entity/items/inventory/interactions/combat/permissions;
- DATA_API.md — configuration, catalogs, persistence, diagnostics, ABI;
- WORLD_SYNC.md — current world-to-client synchronization mechanism;
- WORLD_STORAGE.md — current custom world durability mechanism.

The governing split remains:

> PHP owns gameplay semantics and developer/plugin experience. Rust/native code owns mechanisms, concurrency infrastructure, networking, wire representation, native state, persistence machinery, and measured hot paths.

## Current subsystem inventory

### Server

Status: implemented foundation.

Current source:

~~~text
src/Server
~~~

Responsibilities:

- public composition/lifecycle facade;
- graceful stop and shutdown ordering;
- direct event/command/task/plugin operations;
- ownership of the internal owner-runtime boundary.

Target design: RUNTIME_API.md.

Current direction:

- construction remains side-effect free until explicit start();
- `Server` is a public facade rather than the session packet/tick implementation, and `Server::run()` is the normal execution entrypoint;
- `Server\Internal\Runtime` owns native-session routing, bootstrap/gameplay dispatch, tick ordering, and world maintenance;
- plugin-owned registration lifetime is integrated;
- readonly `ServerConfig` replaces the primitive creation argument bag;
- remaining tick-boundary batching is performance work, not API cleanup.

### Commands

Status: typed Brigadier-style tree foundation implemented.

Current source:

~~~text
src/Command
~~~

Target design: COMMANDS.md.

Current direction:

- recursive literal/argument trees compile at registration;
- variadic immutable then() composition is public API;
- typed handler injection avoids context-map casts;
- aliases, requirements, basic suggestions, and plugin-owned bindings are implemented;
- basic word/string/greedy/integer/boolean/enum arguments are implemented;
- semantic gameplay arguments plus generated usage/help arrive with their owning subsystems.

### Events

Status: implemented owner-runtime dispatcher.

Current source:

~~~text
src/Event
~~~

Target design: RUNTIME_API.md.

Current direction:

- typed event subscriptions return Subscription handles;
- concrete listener chains are cached instead of rescanning every registered event type;
- plugin-owned subscriptions are released deterministically;
- focused cancellable events and explicit priority ordering remain future event-model work.

### Tasks / scheduler

Status: implemented delayed work, repeating work, Fibers, native awaits.

Current source:

~~~text
src/Task
~~~

Target design: RUNTIME_API.md.

Current direction:

- scheduling returns TaskHandle objects;
- task() starts Fibers on the scheduler boundary rather than inline;
- due-time heap, sleep marker, and native-await marker implementations live under `Task\Internal`;
- raw async-ready/take FFI calls are centralized in `Cobblestone\Native\Tasks`;
- cancellation is explicit and failed/repeating work does not remain active;
- batched native-ready completion retrieval remains separate performance work.

### Plugins

Status: owned plugin lifecycle foundation implemented.

Current source:

~~~text
src/Plugin
~~~

Target design: RUNTIME_API.md.

Current direction:

- PluginScope owns events, commands, tasks, and cleanup;
- failed enable rolls back partial registrations;
- unload/shutdown cleanup is deterministic;
- the old service-locator-style PluginContext is removed;
- closure-first entrypoints and dependency metadata remain separate plugin-system work.

### Logging

Status: implemented PSR-3 / Monolog logging.

Current source:

~~~text
src/Log
~~~

Target design: RUNTIME_API.md and DATA_API.md.

Target direction:

- keep PSR-3;
- plugin/subsystem contextual loggers;
- structured context underneath console formatting;
- bounded/rate-aware hot-path diagnostics.

### Session / bootstrap

Status: implemented fixed-target production join/session path.

Current source:

~~~text
src/Session/Event
src/Session/Internal
src/Native/Session.php
native/session
~~~

Target design: TRANSPORT_API.md.

Current direction:

- semantic PHP session events remain under `Session\Event`;
- server-owned bootstrap/gameplay/view-preparation state lives under `Session\Internal` and is not plugin API;
- `Cobblestone\Native\Session` is the sole PHP adapter for raw native session FFI;
- Session remains connection/bootstrap identity, not Player;
- bounded pollBatch-style owner boundary remains future performance work;
- preserve per-session ordering while moving measured CPU work off the single session owner.

### Network

Status: implemented protocol-8 RakNet backend.

Current crate:

~~~text
native/transport
~~~

Target design: TRANSPORT_API.md.

Target direction:

- remain mechanism-only;
- bounded command/event queues;
- explicit backpressure/disconnect policy;
- avoid central-loop head-of-line blocking if measurements confirm it;
- no RakNet objects in plugin API.

### Protocol codec

Status: implemented protocol-84 codec/bootstrap/batch foundation.

Current crate:

~~~text
native/protocol84
~~~

Target design: TRANSPORT_API.md.

Target direction:

- typed packet/frame structures;
- decode outer frames once;
- bounded malformed-input handling;
- cheap immutable buffer sharing;
- compression placement determined by measurements.

### Native runtime / FFI

Status: implemented extension bridge and native mechanism surface.

Current crate:

~~~text
native/extension
~~~

Related mechanisms:

~~~text
native/runtime
native/world
~~~

Target design: API_STYLE.md, RUNTIME_API.md, TRANSPORT_API.md, DATA_API.md, WORLD_API.md.

Current direction:

- the PHP world FFI boundary is centralized in `Cobblestone\Native\World`; semantic World code contains no raw `cobblestone_world_*` calls;
- the PHP session FFI boundary is centralized in `Cobblestone\Native\Session`, and scheduler completion probes in `Cobblestone\Native\Tasks`;
- ABI v1 replaces per-export capability probing for native adapters;
- typed projections/results stay under narrow `Cobblestone\Native\*` namespaces while 1:1 forwarding classes are removed;
- owner-safe handles remain native implementation detail;
- further batching and export-count reduction remain separate performance/API work;
- no arbitrary Zend calls from worker threads.

### Worker/completion runtime

Status: implemented bounded native worker pool.

Current crate:

~~~text
native/runtime
~~~

Target design: RUNTIME_API.md.

Target direction:

- bounded queues;
- immutable/owned job inputs;
- cooperative/native cancellation;
- completion delivery to owner runtime;
- no public generic plugin thread pool.

### World semantic model

Status: implemented foundation.

Current source:

~~~text
src/World
~~~

Target design: WORLD_API.md.

Target direction:

- World as semantic root;
- direct block/chunk/time/spawn operations;
- WorldEdit for coarse mutation;
- scoped residency/ChunkLease rather than residency-cell exposure;
- stable typed coordinate/state values.

### World generation

Status: implemented Flat generation foundation, exact fixed-target Overworld biome source, and exact Infinite Overworld base-shape + surface-building + cave-carving + lake-population + Village/Mineshaft/Stronghold/Scattered structure + common ore-decoration foundations; remaining dungeon/biome features plus Old/Nether generation remain pending. Ocean Monument is post-target 0.16 content and is intentionally absent.

Current source:

~~~text
src/World/Generator
~~~

Target design: WORLD_API.md.

Target direction:

- small Generator contract;
- biome selection is a separate `BiomeSource` semantic boundary from terrain material generation;
- `OverworldBiomeSource` samples the recovered 0.15.10 layered source natively and returns compact immutable `BiomeArea` byte planes through one coarse FFI call;
- native `OverworldTerrainShape` implements the exact `RandomLevelSource::prepareHeights` stage as one 16×128×16 immutable state projection using the raw pre-Voronoi biome layer;
- native `OverworldSurfaceBuilder` composes that shape with final 1:1 biomes, the target four-octave surface simplex, chunk-seeded MT state, bedrock, and exact biome top/filler overrides into one immutable surfaced chunk;
- native `OverworldCaveCarver` applies the target `LargeCaveFeature` pass over the surfaced chunk, including the radius-8 source-chunk scan, MT reseeding, tunnel/room recursion, water-abort mutation, lava cutoff, sand repair, and grass repair;
- transient native `PopulationNeighborhood` owns the target 3x3 post-process write boundary so population features may mutate neighboring chunks without clipping;
- `OverworldLakePopulator` applies the target pre-structure water/lava `LakeFeature` stage to that shared neighborhood, including desert water exclusion, failed-water lava suppression, fixed-target cavity validation, and cross-chunk writes;
- reusable native `StructureStartCore` + `StructureStartCache<T>` own cached-start/bounds/per-chunk idempotence mechanics; `VillageStructureState` / `MineshaftStructureState` / `StrongholdStructureState` / `ScatteredStructureState` keep family-specific durable pieces outside `PopulationNeighborhood`, while their Overworld structure owners implement the four fixed-target structure stages and expose crate-internal shared-random post-process paths;
- `OverworldMonsterRoomPopulator` continues that population RNG stream for the eight fixed-target dungeon attempts, mutating the same 3x3 neighborhood while preserving commented-out chest-loot/spawner-entity semantics;
- `OverworldFreezeFrostPopulator` applies the executable-verified 0.15.10 center 16x16 water-to-ice pass next, including fixed cold-biome temperatures, Y=127 rain-height quirks, and the zero pre-light block-light boundary; it deliberately places no top snow;
- `OverworldOreDecorator` retains the isolated common `BiomeDecorator::decorateOres` + Mesa-extra-gold mechanism, while `OverworldBiomeDecorator` owns the actual post-freeze independently reseeded `Biome::decorate` stream and continues it through all recovered feature/tree families and biome-specific hooks using the cached pre-population heightmap;
- `OverworldPostDecorationFinalizer` completes the recovered target stage set with perimeter water repair, generation-time Seasons/top-snow, generation-only liquid tick draining, final height recomputation, and packed sky/block-light output over the shared 3x3 neighborhood;
- Ocean Monument is explicitly excluded from the 0.15.10 parity surface: target runtime/registry evidence is absent and Mojang introduced the live feature in 0.16;
- native `OverworldInfiniteGenerator` composes the complete recovered pipeline over authoritative 3x3 inputs; `WorldFactory::infinite()` and `WorldFactory::persistentInfinite()` expose it publicly, with native lifecycle/light installation, durable generator-owned structure state, and representative whole-pipeline fixtures;
- Flat generation remains preset/fixed-biome driven and does not route through the Overworld source;
- mutable generation-owned ChunkDraft;
- whole-chunk/coarse commit rather than thousands of scalar owner/native calls;
- deterministic seed/coordinate behavior.

### World lighting

Status: implemented fixed-target lighting foundation.

Current source:

~~~text
src/World/Light
~~~

Target design: WORLD_API.md.

Target direction:

- lighting follows semantic world mutation automatically;
- no ordinary LightManager API;
- measured propagation mechanism may move/batch natively;
- light state remains part of snapshots/protocol projection.

### World mutation

Status: implemented staged mutation foundation.

Current source:

~~~text
src/World/Mutation
~~~

Target design: WORLD_API.md.

Target direction:

- WorldEdit is public semantic boundary;
- validate/apply/coalesce once;
- incremental height maintenance;
- viewer change publication separate from persistence.

### Native world store

Status: implemented region-sharded native world state.

Current crate area:

~~~text
native/world/src/*
~~~

Target design: WORLD_API.md plus WORLD_SYNC.md.

Target direction:

- preserve owner/runtime rules;
- evaluate section-level COW if benchmarks justify it;
- explicit dirty candidate queues;
- coarse patch/snapshot/change projection calls.

### World persistence

Status: implemented custom v1 region/chunk durability and async workers.

Current crate:

~~~text
native/storage
~~~

Target design: WORLD_API.md, DATA_API.md, WORLD_STORAGE.md.

Target direction:

- transparent dirty persistence;
- bounded save/load workers;
- dirty queue rather than resident-world scan;
- safe revision/watermark handling;
- compaction remains native maintenance.

## Planned gameplay subsystems

The following systems are expected but not yet full implementation packages.

### Player

Status: planned.

Target design: GAMEPLAY_API.md.

Responsibilities:

- gameplay identity after session bootstrap;
- world/position/game mode;
- inventory and messaging access;
- teleport/kick/gameplay operations;
- persistent profile linkage.

Player is not Session.

### Entity

Status: planned.

Target design: GAMEPLAY_API.md.

Responsibilities:

- stable entity identity;
- spatial/world state;
- spawning/removal;
- bounded spatial queries;
- living/nonliving behavior foundations.

Avoid a deep public subclass hierarchy and avoid committing to public ECS without evidence.

### Movement / physics

Status: planned.

Target design: GAMEPLAY_API.md.

Responsibilities:

- validate MovePlayer-derived intent;
- collision/position rules;
- authoritative movement commit;
- semantic movement events;
- viewer publication.

Raw packets are not the gameplay API.

### Block gameplay behavior

Status: partial state model exists; behavior planned.

Target design: WORLD_API.md and GAMEPLAY_API.md.

Responsibilities:

- placement;
- breaking;
- activation;
- drops;
- tool/item interaction;
- block updates.

Stored BlockState remains separate from behavior.

### Items

Status: planned.

Target design: GAMEPLAY_API.md.

Responsibilities:

- ItemType catalog;
- immutable ItemStack values;
- metadata/NBT parity;
- item use behavior.

Avoid one mutable object/subclass per stack.

### Inventory

Status: planned.

Target design: GAMEPLAY_API.md.

Responsibilities:

- authoritative slots;
- atomic InventoryEdit;
- transaction validation;
- player/container inventories;
- protocol synchronization.

### Containers

Status: planned.

Target design: GAMEPLAY_API.md.

Responsibilities:

- one viewer/player container lifetime;
- open/close;
- protocol window mapping;
- atomic slot interactions.

ContainerSession is a lifetime handle.

### Crafting / recipes

Status: planned.

Target design: GAMEPLAY_API.md.

Responsibilities:

- immutable recipe definitions;
- compiled matching;
- plugin-owned recipe registration;
- atomic input/output commit.

Implement only recipe forms supported by the fixed target.

### Damage / combat / health

Status: planned.

Target design: GAMEPLAY_API.md.

Responsibilities:

- typed Damage;
- damage/death events;
- health state;
- attack rules;
- drops/respawn/knockback as implemented.

Rules follow 0.15.10 evidence rather than modern Minecraft assumptions.

### Attributes

Status: planned.

Target design: GAMEPLAY_API.md.

Responsibilities:

- typed attribute identities;
- base/current/modifier state where fixed-target gameplay uses it;
- protocol projection.

### Effects

Status: planned.

Target design: GAMEPLAY_API.md.

Responsibilities:

- typed status effects;
- tick-based duration;
- apply/remove/expire;
- gameplay/protocol synchronization.

### Messaging / chat

Status: planned beyond low-level packet support.

Target design: GAMEPLAY_API.md.

Responsibilities:

- player messages;
- server broadcast;
- validated chat flow;
- semantic chat events;
- structured Text only when needed.

### Permissions

Status: planned.

Target design: GAMEPLAY_API.md.

Responsibilities:

- permission identity;
- subject resolution;
- command/gameplay integration;
- plugin-owned registrations/temporary attachments;
- cached invalidatable checks.

No second command-specific permission system.

### Scoreboard / presentation features

Status: reserved, protocol-gated.

Target design: GAMEPLAY_API.md.

Responsibilities only when fixed-target support is proven:

- scoreboard-like state;
- titles/action presentation;
- other client presentation surfaces.

Do not create empty managers before protocol/gameplay work begins.

## Planned data/runtime support

### Server configuration

Status: readonly ServerConfig foundation implemented.

Target design: DATA_API.md.

Current direction:

- Server::create() accepts typed ServerConfig rather than a primitive constructor bag;
- runtime dependencies such as World and LoggerFactory stay outside config;
- configuration source precedence remains composition-root work;
- explicit runtime changes are required for settings that become mutable.

### Fixed-target catalogs

Status: exact block and biome catalog foundations implemented; remaining catalog domains planned.

Target design: DATA_API.md.

Catalog domains may include:

~~~text
BlockType
ItemType
EntityType
BiomeType
EffectType
AttributeType
packet identifiers
~~~

Protocol IDs stay attached to catalog entries rather than leaking through normal gameplay code.

### Player persistence

Status: planned.

Target design: DATA_API.md.

Target direction:

- immutable PlayerData snapshot;
- background load/save;
- live Player objects remain owner-runtime only;
- stable persistent identity separate from session identity.

### Entity persistence

Status: planned.

Target design: DATA_API.md.

Only entities whose lifecycle requires durability are persisted.

Temporary runtime entities should not become durable accidentally.

### Plugin data

Status: planned.

Target design: DATA_API.md.

Target direction:

- plugin namespace isolation;
- atomic/versioned data;
- migrations;
- no generic ORM invented ahead of a use case;
- no direct access to native world region files.

### Diagnostics / metrics

Status: partial logs/native diagnostics exist; unified surface planned.

Target design: DATA_API.md.

Target direction:

- cheap counters/gauges/timings;
- immutable status snapshots;
- queue/tick/session/world/storage visibility;
- adapters for external metrics systems later;
- no management mutation through generic diagnostic property setters.

### Native ABI capabilities

Status: versioned ABI identity implemented; typed result mapping started.

Target design: DATA_API.md.

Current direction:

- `cobblestone_core_abi()` publishes ABI version 1;
- PHP validates ABI once at native runtime startup instead of probing every export;
- prepared-view send status is mapped to `ViewSendResult`;
- add capability metadata only when optional native features actually require it;
- continue centralizing other native result codes as touched.

## Protocol-gated/reserved areas

These areas should be modeled only after pinned protocol/source evidence shows what 0.15.10 actually requires.

Examples include:

- resource-pack negotiation;
- skin/client metadata beyond current login needs;
- additional presentation UI;
- dimension identity/persistence/bootstrap projection is implemented; dimension-specific generation, portals, transfer, and broader environment behavior remain unimplemented;
- advanced authentication/identity verification.

The subsystem map reserves architectural space without claiming unsupported functionality.

## Candidate future domain boundaries

Possible future PHP namespaces/directories include:

~~~text
src/Player
src/Entity
src/Item
src/Inventory
src/Permission
~~~

These are candidates, not preapproved empty packages.

A package should be created when:

- the subsystem has implementation;
- it owns coherent public types/behavior;
- its dependencies can remain one-way;
- the split improves ownership or independent testing.

Movement, combat, effects, crafting, chat, or registries may live inside the nearest semantic package until they prove they deserve a separate package.

Avoid a catch-all gameplay package merely to hold unrelated future code.

## Candidate native boundaries

Do not create native crates for gameplay taxonomy.

Potential native/mechanism work remains evidence-driven, for example:

- entity broad-phase/spatial indexing;
- collision hot loops;
- compression;
- compact protocol projection;
- persistent data encoding/I/O;
- bulk catalog lookup.

If a mechanism does not need an independent version/dependency boundary, it may stay in an existing crate.

## Dependency direction

The intended high-level dependency direction is:

~~~text
fixed-target catalogs / values
        ↓
world + gameplay primitives
        ↓
player/entity/inventory semantics
        ↓
session/bootstrap + server composition
        ↓
plugins use the public semantic surface
~~~

Native mechanisms sit underneath the relevant semantic package through the extension boundary.

Plugin code must not become a dependency of core gameplay packages.

## Definition of subsystem completion

A subsystem is not complete merely because its classes exist.

Completion requires:

- semantic API defined;
- ownership/lifetime defined;
- failure behavior defined;
- plugin ownership/cleanup defined where registrations exist;
- fixed-target parity established where relevant;
- tests run against the relevant revision;
- hot paths measured where performance claims matter;
- docs and implementation agree;
- remote repository state verified after mutation.

## Design summary

Cobblestone's full server direction is now covered by one coherent set of API documents.

Existing systems should converge without unnecessary rewrites.

Missing systems should be implemented against these semantic boundaries instead of inventing manager-heavy APIs ad hoc.

The recurring rule is:

> Keep gameplay simple and typed in PHP. Keep machinery bounded and explicit underneath. Do not expose the architecture diagram to plugin authors.
