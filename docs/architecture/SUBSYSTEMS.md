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

Current package:

~~~text
modules/php/server
~~~

Responsibilities:

- composition root;
- lifecycle/tick loop;
- graceful stop;
- server-owned session/world/gameplay orchestration.

Target design: RUNTIME_API.md.

Primary convergence work:

- construction must not emit Started before registration is possible;
- expose direct semantic operations rather than manager chains;
- integrate plugin-owned registration/lifetime;
- keep tick work bounded.

### Commands

Status: implemented simple registry/dispatch; redesign specified.

Current package:

~~~text
modules/php/command
~~~

Target design: COMMANDS.md.

Target direction:

- real recursive Brigadier-style tree;
- literal and typed argument nodes;
- variadic then();
- typed handler injection;
- suggestions, requirements, aliases, usage/help from one tree;
- registration-time compilation and validation;
- plugin-owned binding lifetime.

### Events

Status: implemented owner-runtime dispatcher.

Current package:

~~~text
modules/php/event
~~~

Target design: RUNTIME_API.md.

Target direction:

- typed event subscriptions;
- Subscription handles;
- compiled concrete listener chains rather than scanning every registered type;
- focused cancellable events;
- deterministic priority/order;
- plugin-owned lifetime.

### Tasks / scheduler

Status: implemented delayed work, repeating work, Fibers, native awaits.

Current package:

~~~text
modules/php/task
~~~

Target design: RUNTIME_API.md.

Target direction:

- TaskHandle objects instead of integer IDs;
- task() starts on scheduler boundary rather than executing Fiber inline;
- due-time heaps remain;
- batched native-ready completion retrieval;
- explicit cooperative cancellation semantics.

### Plugins

Status: implemented class-based load/enable/disable foundation.

Current package:

~~~text
modules/php/plugin
~~~

Target design: RUNTIME_API.md.

Target direction:

- PluginScope owns registrations/resources;
- enable rollback on failure;
- deterministic unload;
- closure-first entrypoint supported alongside class entrypoints;
- dependency metadata resolved before enable;
- no service-locator-style plugin context.

### Logging

Status: implemented PSR-3 / Monolog logging.

Current package:

~~~text
modules/php/log
~~~

Target design: RUNTIME_API.md and DATA_API.md.

Target direction:

- keep PSR-3;
- plugin/subsystem contextual loggers;
- structured context underneath console formatting;
- bounded/rate-aware hot-path diagnostics.

### Session / bootstrap

Status: implemented fixed-target production join/session path.

Current package:

~~~text
modules/php/session
modules/rust/session
~~~

Target design: TRANSPORT_API.md.

Target direction:

- Session remains connection/bootstrap identity, not Player;
- bounded pollBatch-style owner boundary;
- one decode path per frame;
- host-local session state where useful;
- preserve per-session ordering while moving measured CPU work off the single session owner.

### Network

Status: implemented protocol-8 RakNet backend.

Current crate:

~~~text
modules/rust/network
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
modules/rust/codec
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
modules/rust/php-extension
~~~

Related mechanisms:

~~~text
modules/rust/core
~~~

Target design: API_STYLE.md, RUNTIME_API.md, TRANSPORT_API.md, DATA_API.md, WORLD_API.md.

Target direction:

- coarse semantic native calls;
- one ABI/capability contract instead of a growing function_exists list;
- centralized native result mapping;
- owner-safe handles;
- batched completions/session/world operations;
- no arbitrary Zend calls from worker threads.

### Worker/completion runtime

Status: implemented bounded native worker pool.

Current crate:

~~~text
modules/rust/core
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

Current package:

~~~text
modules/php/world
~~~

Target design: WORLD_API.md.

Target direction:

- World as semantic root;
- direct block/chunk/time/spawn operations;
- WorldEdit for coarse mutation;
- scoped residency/ChunkLease rather than residency-cell exposure;
- stable typed coordinate/state values.

### World generation

Status: implemented Flat generation foundation.

Current package:

~~~text
modules/php/world-generation
~~~

Target design: WORLD_API.md.

Target direction:

- small Generator contract;
- mutable generation-owned ChunkDraft;
- whole-chunk/coarse commit rather than thousands of scalar owner/native calls;
- deterministic seed/coordinate behavior.

### World lighting

Status: implemented fixed-target lighting foundation.

Current package:

~~~text
modules/php/world-light
~~~

Target design: WORLD_API.md.

Target direction:

- lighting follows semantic world mutation automatically;
- no ordinary LightManager API;
- measured propagation mechanism may move/batch natively;
- light state remains part of snapshots/protocol projection.

### World mutation

Status: implemented staged mutation foundation.

Current package:

~~~text
modules/php/world-mutation
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
modules/rust/core/src/world*
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
modules/rust/storage
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

Status: partial environment/config composition exists.

Target design: DATA_API.md.

Target direction:

- parse once into readonly typed ServerConfig;
- deterministic source precedence;
- no scattered steady-state getenv calls;
- explicit runtime changes for mutable settings.

### Fixed-target catalogs

Status: partial protocol/world constants exist; unified catalog design planned.

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

Status: current PHP runtime probes many function names manually.

Target design: DATA_API.md.

Target direction:

- one versioned ABI/capability identity;
- validate once at startup;
- centralize native result-code mapping.

## Protocol-gated/reserved areas

These areas should be modeled only after pinned protocol/source evidence shows what 0.15.10 actually requires.

Examples include:

- resource-pack negotiation;
- skin/client metadata beyond current login needs;
- additional presentation UI;
- dimensions/environment features not yet implemented;
- advanced authentication/identity verification.

The subsystem map reserves architectural space without claiming unsupported functionality.

## Candidate future package boundaries

Possible future PHP packages include:

~~~text
modules/php/player
modules/php/entity
modules/php/item
modules/php/inventory
modules/php/permission
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
