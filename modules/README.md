# Modules

Cobblestone product code lives under one top-level module boundary:

- `modules/php/` — PHP server/domain code, autoloaded as `Cobblestone\\...`.
- `modules/rust/` — Rust mechanism crates and the PHP native extension.

The repository root contains orchestration, configuration, documentation, tests, and tooling only. There is intentionally no product `src/` or `native/` directory at the repository root.

PHP directories describe responsibilities (`Server`, `Session`, `Event`, `Command`, `Task`, `Native`, `Plugin`) rather than a generic Kernel/Core bucket. Rust crates keep their local Cargo-standard `src/` directories.

This layout does not create a runtime module loader and does not affect Zend function registration. Native PHP exports are owned by `modules/rust/php-extension` and verified independently.
