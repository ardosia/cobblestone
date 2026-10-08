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
spec/
├── target.toml
├── blocks.toml
└── biomes.toml

src/
├── Cobblestone.php (PHP launcher)
├── Application/
├── Config/
├── Command/
├── Event/
├── Log/
├── Native/
├── Plugin/
├── Server/
├── Session/
├── Task/
└── World/

native/
├── target/
├── runtime/
├── world/
├── wire/
├── raknet/
├── session/
├── storage/
└── extension/

build/
└── src/ (Rust xtask driver)

tests/php/
├── unit/
├── native/
├── integration/
├── fixtures/
└── shared bootstrap/probe helpers
```

PHP is one Composer package. The root autoloader maps `Cobblestone\\` to `src/`. Public API is semantic; implementation plumbing lives under explicit `Internal` or `Native` boundaries.

The native workspace is split by mechanism ownership:

- `target`: generated flat API for cross-language fixed-target identities/layout from `spec/`
- `runtime`: runtime identity, generational handles, ownership epochs, and region routing
- `world`: authoritative native chunk/world state, revisions, residency, snapshots, patches
- `wire`: fixed-target packet, NBT, Batch, chunk, login, and gameplay wire logic
- `raknet`: fixed-target RakNet / UDP reliability and connection transport
- `session`: ordered native connection/session host
- `storage`: world metadata, region files, async load/save, compaction
- `extension`: narrow Zend bridge; the PHP module/library ABI name remains `cobblestone_core_php`

## Runtime shape

`Server` is the ordinary public facade. Construction is side-effect free; `Server::run()` owns normal execution and delegates tick/session mechanics behind internal runtime boundaries.

Commands, events, plugins, tasks, and world operations are exposed as direct semantic operations rather than manager/registry chains. Server-owned session bootstrap/gameplay state and scheduler implementation markers are internal.

`World` owns fixed 16×16×128 MCPE 0.15.10 world semantics. Generation, lighting, and mutation live under the same World domain. Every production `Chunk` is a PHP semantic facade over the authoritative native `WorldStore`; world creation requires `cobblestone_core_php` instead of switching to a second PHP chunk backend.

The PHP/native boundary stays coarse. Native snapshots and atomic patches avoid per-cell marshaling where possible; world/session/task raw FFI calls are centralized under `Cobblestone\Native`.

## Persistence

The production application entry point is `src/Cobblestone.php`; it uses the MCPE 0.15.10-targeted Infinite Overworld generator with the custom persistent world store at `worlds/world` by default. Compatibility-sensitive generation is driven by executable/offline-world evidence rather than modern Minecraft behavior; unresolved parity gaps stay in GitHub Issues instead of being hidden in documentation.

Useful environment variables:

- `COBBLESTONE_WORLD_DIR`
- `COBBLESTONE_WORLD_NAME`
- `COBBLESTONE_WORLD_SEED` — MCPE 0.15.10 seed-box input (numeric or text); unset/single-character input uses a random 32-bit seed
- `COBBLESTONE_VIEW_DISTANCE` — maximum client chunk radius 1..3 (default 3; a 7×7 chunk view when the client requests at least 3)
- `COBBLESTONE_SAVE_WORKERS`
- `COBBLESTONE_LOAD_WORKERS`
- `COBBLESTONE_COMPACTION_MIN_DEAD_BYTES`
- `COBBLESTONE_COMPACTION_MIN_DEAD_PERCENT`

Stored `world.cwm` metadata wins over creation defaults when reopening a world. Chunk persistence retains fixed-target block/data/light/biome planes, sparse extra data, and generated chest block-entity state. Save/load worker counts must be 1..32. Region compaction is gated by configured reclaimable-dead-byte and dead-percent thresholds.

See `docs/architecture/WORLD_STORAGE.md` for the storage model and `docs/architecture/WORLD_SYNC.md` for chunk/view synchronization behavior.

## Developer workflow

The repository driver is Rust `xtask`. On first checkout or after PHP dependency changes:

```text
cargo +1.98.0 xtask setup
```

Common commands:

```text
cargo +1.98.0 xtask modules
cargo +1.98.0 xtask build
cargo +1.98.0 xtask check
cargo +1.98.0 xtask test
cargo +1.98.0 xtask verify
cargo +1.98.0 xtask serve
```

Focused native/PHP commands:

```text
cargo +1.98.0 xtask native-check
cargo +1.98.0 xtask native-build
cargo +1.98.0 xtask test-php
```

Composer keeps matching aliases such as `composer verify` and `composer serve` for convenience; it is no longer the orchestration implementation. Benchmarks remain available as `composer bench:scheduler` and `composer bench:storage`. `cargo +1.98.0 xtask setup` installs dependencies from the committed Composer lock file, while `cargo +1.98.0 xtask verify` is the canonical broad local validation command. GitHub Actions run the Ubuntu quality gate and a dedicated Windows compatibility job on pull requests, pushes to `main`, and manual dispatches. The Windows job exercises the pinned nightly PHP adapter plus the same ZTS/native PHP integration surface instead of living behind a manual-only matrix.

## Architecture

Durable engineering design lives under `docs/architecture/`. Start with:

- `FOUNDATION.md`
- `API_STYLE.md`
- `RUNTIME_API.md`
- `COMMANDS.md`
- `GAMEPLAY_API.md`
- `WORLD_API.md`
- `WORLD_STORAGE.md`
- `WORLD_SYNC.md`
- `TRANSPORT_API.md`
- `DATA_API.md`
- `SUBSYSTEMS.md`

Provenance and compatibility notes live under `docs/provenance/`.

Fixed-target data shared across Rust and PHP is authored once under `spec/` and compiled into committed source with `cargo xtask generate`. `cargo xtask generate --check` verifies that generated target APIs are current.
