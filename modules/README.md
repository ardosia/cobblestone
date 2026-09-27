# Modules

Cobblestone product code lives under one top-level module boundary:

- `modules/php/` — PHP server/domain code, autoloaded as `Cobblestone\\...`.
- `modules/rust/` — Rust mechanism crates and the PHP native extension.

The repository root contains orchestration, documentation, tests, benchmarks, and tooling only. There is intentionally no product `src/` or `native/` directory at repository root.

## PHP

PHP directories describe concrete responsibilities rather than a generic Kernel/Core/Internal bucket:

```text
modules/php/
├── Server/
├── Session/
├── Event/
├── Command/
├── Task/
├── Native/
└── Plugin/
```

Gameplay domains such as World, Player, Entity, Block, and Inventory are added only when their implementation begins.

## Rust

Rust crate identities remain stable while repository paths become concise:

```text
modules/rust/
├── core/
├── codec/
├── network/
├── session/
└── php-extension/
```

Each crate keeps normal Cargo-local `src/` directories. Large source files are split only at real responsibility seams: core arena/worker internals, codec packet/NBT model-vs-codec logic, session values/server/wire/host runner, network backend surface-vs-runner, and PHP-extension boundary/runtime/diagnostics/session join.

The repository-level `modules/` directory is not a runtime module loader. Composer class paths do not control Zend exports; the extension under `modules/rust/php-extension` owns and tests those names independently.
