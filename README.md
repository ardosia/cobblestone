# Cobblestone

Cobblestone is a fixed-target Minecraft Windows 10 Edition Beta / MCPE 0.15.10 server.

Target:

- game protocol **84**
- RakNet protocol **8**
- gameplay/plugin layer **PHP 8.5 ZTS**
- native mechanisms and measured hot paths **Rust**

PHP owns gameplay semantics and the ordinary plugin/developer API. Rust owns transport, wire representation, concurrency infrastructure, native state, persistence, and measured hot paths.

## Repository layout

Code is organized by responsibility rather than language package bureaucracy:

```text
src/
├── Command/
├── Event/
├── Log/
├── Native/
├── Plugin/
├── Server/
├── Session/
├── Task/
├── Tick/
└── World/

native/
├── runtime/
├── world/
├── protocol84/
├── transport/
├── session/
├── storage/
└── extension/

tests/php/
├── unit/
├── native/
├── integration/
├── fixtures/
└── shared bootstrap/probe helpers
```

PHP is one Composer package. The root autoloader maps `Cobblestone\\` to `src/`. Public API is semantic; implementation plumbing lives under explicit `Internal` or `Native` boundaries.

The native workspace is split by mechanism ownership:

- `runtime`: handles, ownership, workers, completion primitives, region routing
- `world`: authoritative native chunk/world state, revisions, residency, snapshots, patches
- `protocol84`: fixed-target packet, NBT, Batch, chunk, login, and gameplay wire logic
- `transport`: RakNet 8 / UDP reliability and connection transport
- `session`: ordered native connection/session host
- `storage`: world metadata, region files, async load/save, compaction
- `extension`: narrow Zend bridge; the PHP module/library ABI name remains `cobblestone_core_php`

## Runtime shape

`Server` is the ordinary public facade. Construction is side-effect free; `Server::run()` owns normal execution and delegates tick/session mechanics behind internal runtime boundaries.

Commands, events, plugins, tasks, and world operations are exposed as direct semantic operations rather than manager/registry chains. Server-owned session bootstrap/gameplay state and scheduler implementation markers are internal.

`World` owns fixed 16×16×128 MCPE 0.15.10 world semantics. Generation, lighting, and mutation live under the same World domain. `Chunk` is backend-agnostic through an internal two-implementation state boundary: native `WorldStore` state when the extension is available, or the parity-tested PHP fallback.

The PHP/native boundary stays coarse. Native snapshots and atomic patches avoid per-cell marshaling where possible; world/session/task raw FFI calls are centralized under `Cobblestone\Native`.

## Persistence

The production CLI uses the exact fixed-target Infinite Overworld generator with the custom persistent world store at `worlds/world` by default.

Useful environment variables:

- `COBBLESTONE_WORLD_DIR`
- `COBBLESTONE_WORLD_NAME`
- `COBBLESTONE_WORLD_SEED` — MCPE 0.15.10 seed-box input (numeric or text); unset/single-character input uses a random 32-bit seed
- `COBBLESTONE_VIEW_DISTANCE` — maximum client chunk radius 1..3 (default 3; a 7×7 chunk view when the client requests at least 3)
- `COBBLESTONE_SAVE_WORKERS`
- `COBBLESTONE_LOAD_WORKERS`
- `COBBLESTONE_COMPACTION_MIN_DEAD_BYTES`
- `COBBLESTONE_COMPACTION_MIN_DEAD_PERCENT`

Stored `world.cwm` metadata wins over creation defaults when reopening a world. Save/load worker counts must be 1..32. Region compaction is gated by configured reclaimable-dead-byte and dead-percent thresholds.

See `docs/architecture/WORLD_STORAGE.md` for the storage model and `docs/architecture/WORLD_SYNC.md` for chunk/view synchronization behavior.

## Developer workflow

First checkout or after PHP dependency changes:

```text
composer setup
```

Common commands:

```text
composer modules
composer build
composer check
composer test
composer verify
composer serve
composer bench:scheduler
composer bench:storage
```

Focused native commands:

```text
composer native:check
composer native:build
composer test:php
```

`composer verify` is the broad local validation command. GitHub Actions are currently manual-dispatch only.

## Architecture

Durable engineering design lives under `docs/architecture/`. Start with:

- `FOUNDATION.md`
- `API_STYLE.md`
- `RUNTIME_API.md`
- `COMMANDS.md`
- `WORLD_API.md`
- `TRANSPORT_API.md`
- `DATA_API.md`
- `SUBSYSTEMS.md`

Provenance and compatibility notes live under `docs/provenance/`.
