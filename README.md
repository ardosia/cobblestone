# Cobblestone

Cobblestone is a fixed-target Minecraft Windows 10 Edition Beta / MCPE 0.15.10 server.

Initial compatibility target:

- game protocol: **84**
- RakNet protocol: **8**
- high-level gameplay/plugin language: **PHP 8.5 ZTS**
- native mechanisms and measured hot paths: **Rust/native extensions**

PHP owns gameplay semantics and the ordinary plugin/developer experience. Rust owns mechanisms, concurrency infrastructure, networking, wire representation, native state, and measured hot paths.

## Repository layout

Product code lives under `modules/`:

```text
modules/
├── php/
│   ├── kernel/
│   ├── log/
│   ├── native-session/
│   ├── plugin/
│   ├── server/
│   ├── session/
│   ├── task/
│   ├── world/
│   ├── world-generation/
│   ├── world-mutation/
│   └── world-region/
└── rust/
    ├── core/
    ├── codec/
    ├── network/
    ├── session/
    └── php-extension/
```

There is intentionally no repository-root product `src/` or `native/`. Rust crates keep their normal Cargo-local `src/` directories.

### PHP packages

Each direct child of `modules/php/` is an independent local Composer package with its own `composer.json`. Package and directory identities are lowercase:

```text
modules/php/session
ardosia/cobblestone-session
```

PHP API identities remain idiomatic PascalCase:

```php
use Cobblestone\Session\JoinFlow;
use Cobblestone\Native\Session\Runtime;
use Cobblestone\Server\Server;
use Cobblestone\World\World;
use Cobblestone\World\Generator\FlatGenerator;
```

Package roots are source roots; package-local `src/` directories are intentionally not used.

The root Composer project consumes these packages through a `modules/php/*` path repository and composes the application through `ardosia/cobblestone-server`. It does not provide a second catch-all production PSR-4 mapping.

Native Zend exports are registered by `modules/rust/php-extension`; Composer package layout cannot rename them.

The base gameplay package is `modules/php/world`: it owns the fixed 0.15.10 16×16×128 world/chunk model plus the generator, mutation, and execution-region contracts exposed by `World`. Concrete Flat generation, staged mutation, and execution-region mapping live in `world-generation`, `world-mutation`, and `world-region`, each depending one-way on the base world package. The server package is the composition root and assembles the default world graph through `WorldFactory`; `World` itself does not construct concrete collaborators.

`modules/php/kernel` owns the small owner-runtime command/event primitives consumed together by plugins and the server. `modules/php/log` provides the PSR-3/Monolog logging implementation with a Spring Boot-inspired console layout and no banner. Monotonic tick pacing and overload warnings live with the server package that exclusively owns that lifecycle mechanism. The owner-runtime task scheduler keeps scheduled tasks and `TickSleep` Fibers in stable due-time min-heaps, so dormant work does not get scanned every tick; only currently outstanding native awaits still require per-task polling. The server runner handles graceful stop requests/signals instead of leaving a raw infinite loop in the executable.

The production join path now bulk-projects immutable PHP `World` chunk snapshots into the Rust protocol-84 layered chunk encoder and sends a bounded initial radius around the real Flat spawn. The former synthetic spawn-probe export remains registered only for native ABI compatibility; production server composition does not call it.

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
```

Focused native commands:

```text
composer native:check
composer native:build
composer test:php
```

`composer modules` lists the local PHP packages and Rust crates.

Automatic GitHub Actions runs are temporarily disabled while the current repository/package cleanup is validated locally. The workflow remains available through manual dispatch.

## Engineering state

Durable project state lives under `.agent/`; architecture is documented in `docs/architecture/FOUNDATION.md`.

C001-C007 are complete. The PHP package workspace and initial Flat world API were locally verified on PHP 8.5.11 ZTS at revision `9f4786380afe307d8b4c6eb610b6f449a7e6a72f`. The logging/tick/shutdown/mutation/region foundation is locally verified on PHP 8.5.11 ZTS at revision `ff5411bdff963b976d6a2232b62c288249d7fe94`. The fixed 0.15.10 client has joined through the production PHP-owned server/session stack. `world-protocol84-stream-v1` is complete: real PHP Flat chunk snapshots are encoded as protocol-84 `ORDER_LAYERED` terrain, the real client renders the expected horizontal Flat world, and local `composer verify` passed on revision `3fba73581de40e1c22bed40be5b6db695ba2b39d`.