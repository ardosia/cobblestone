# Cobblestone

Cobblestone is a fixed-target Minecraft Windows 10 Edition Beta / MCPE 0.15.10 server.

Initial compatibility target:

- game protocol: **84**
- RakNet protocol: **8**
- high-level gameplay/plugin language: **PHP 8.5 ZTS**
- native mechanisms and measured hot paths: **Rust/native extensions**

PHP owns gameplay semantics and the ordinary plugin/developer experience. Rust owns mechanisms, concurrency infrastructure, networking, wire representation, native state, and measured hot paths.

## Repository layout

Product code is split by responsibility, not by PHP package identity:

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
```

PHP is one Composer package. The root autoloader maps `Cobblestone\\` directly to `src/`; namespaces define semantic domains and no subsystem is separately versioned or installed.

Native mechanisms live under `native/`. Protocol 84 and RakNet transport have explicit crate identities, while the extension remains the narrow Zend bridge and keeps the `cobblestone_core_php` module/library ABI name.

`src/World` owns fixed 0.15.10 16×16×128 world/chunk semantics. Generation, lighting, and mutation live under `src/World/Generator`, `src/World/Light`, and `src/World/Mutation` as subdomains of the same semantic world layer rather than separate packages. The server composition root creates a region-sharded native `WorldStore` when the extension is available; PHP `Chunk` objects remain owner-runtime semantic facades over native terrain/light planes, revisions, lifecycle metadata, immutable snapshots, pin counts, dirty watermarks, and atomic patches. Scalar `BlockStateId` values are the hot-path state currency; `BlockState` is the ergonomic wrapper. `docs/provenance/WORLD_API_PARITY.md` tracks semantic parity against the pinned Ardosia world substrate and records the deliberate runtime adaptations.

`src/Command`, `src/Event`, `src/Plugin`, `src/Task`, and `src/Server` are semantic namespaces inside the single Composer package. `src/Log` provides the PSR-3/Monolog implementation. Monotonic tick pacing remains under `src/Tick`, while the owner-runtime task scheduler keeps scheduled tasks and sleeping Fibers in stable due-time min-heaps; its heap/await marker types live under `Task\\Internal`, so dormant work is not scanned every tick. `Server::run()` owns the normal tick lifecycle and delegates signal/shutdown mechanics to an internal runner instead of exposing execution plumbing to the CLI.

The production join path reads immutable native world snapshots directly inside the extension, reuses revision-keyed protocol-84 FullChunkData encodings, and sends one bounded initial-radius Batch around the real Flat spawn. Persistent joins now prepare the requested view through one reusable native load batch and defer spawn across server ticks until each chunk is either imported from durable storage or confirmed missing and generated; no disk wait or per-chunk polling loop runs on the PHP owner runtime. Live mutations stay on the same coarse boundary: native patch commits append to a bounded change journal, one flush call per server tick coalesces/routes viewer updates, small block-only changes use protocol-84 UpdateBlock, and complete chunk snapshots are used when light/biome/extra-data or large terrain changes require them. PHP still owns gameplay semantics and never marshals per-block network deltas. Obsolete synthetic bootstrap compatibility exports were removed when native ABI version 1 became explicit. `docs/architecture/WORLD_SYNC.md` records the measured thresholds. `native/storage` implements `world.cwm`, the custom v1 chunk-record/dual-index region durability core, and bounded native async save/load services described in `docs/architecture/WORLD_STORAGE.md`. Loaded native chunks are adopted into PHP residency without rewriting terrain, while unresolved synchronous access raises `ChunkLoadPending` instead of clobbering unknown durable state.

## Developer workflow

First checkout or after PHP dependency-graph changes:

```text
composer setup
```

Normal commands:

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

`composer modules` reports the PHP source root and current Rust crates.

The production CLI uses the custom persistent world store by default at `worlds/world` (ignored by Git). `COBBLESTONE_WORLD_DIR` selects another directory; `COBBLESTONE_WORLD_NAME`, `COBBLESTONE_WORLD_SEED`, and `COBBLESTONE_FLAT_PRESET` are creation defaults only and stored `world.cwm` metadata wins on reopen. `COBBLESTONE_SAVE_WORKERS` and `COBBLESTONE_LOAD_WORKERS` select 1..32 native storage workers and default to 2 each. Production region compaction uses a conservative default gate of at least 64 MiB reclaimable dead records and at least 50% dead record bytes. `COBBLESTONE_COMPACTION_MIN_DEAD_BYTES` and `COBBLESTONE_COMPACTION_MIN_DEAD_PERCENT` override those gates; zero disables that criterion, both zero disable scheduling, and every enabled criterion must match before native storage workers queue a compaction.

Automatic GitHub Actions runs are temporarily disabled while the current repository/package cleanup is validated locally. The workflow remains available through manual dispatch.

## Engineering state

Architecture is documented in `docs/architecture/FOUNDATION.md`.

The PHP package workspace and initial Flat world API were locally verified on PHP 8.5.11 ZTS at revision `9f4786380afe307d8b4c6eb610b6f449a7e6a72f`. The logging/tick/shutdown/mutation/region foundation is locally verified on PHP 8.5.11 ZTS at revision `ff5411bdff963b976d6a2232b62c288249d7fe94`. The fixed 0.15.10 client has joined through the production PHP-owned server/session stack. `world-protocol84-stream-v1` is complete: real PHP Flat chunk snapshots are encoded as protocol-84 `ORDER_LAYERED` terrain, the real client renders the expected horizontal Flat world, and local `composer verify` passed on revision `3fba73581de40e1c22bed40be5b6db695ba2b39d`.