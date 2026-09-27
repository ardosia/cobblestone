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
│   ├── command/
│   ├── event/
│   ├── native-session/
│   ├── plugin/
│   ├── server/
│   ├── session/
│   ├── task/
│   └── world/
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

The first gameplay package is `modules/php/world`. It models the fixed 0.15.10 16×16×128 chunk/world surface in PHP and currently implements Flat generation only. The existing synthetic native chunk probe remains in place until the next integration slice projects real `World` chunks to protocol 84.

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

C001-C007 are complete. The fixed 0.15.10 client has joined through the production PHP-owned server/session stack. Repository and package cleanup remains implemented-but-unverified until the requested local validation runs against the persisted revision.