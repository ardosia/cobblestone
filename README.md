# Cobblestone

Cobblestone is a fixed-target Minecraft Windows 10 Edition Beta / MCPE 0.15.10 server.

Initial compatibility target:

- game protocol: **84**
- RakNet protocol: **8**
- high-level gameplay/plugin language: **PHP 8.5 ZTS**
- native mechanisms and measured hot paths: **Rust/native extensions**

PHP owns gameplay semantics and the ordinary plugin/developer experience. Rust owns mechanisms, concurrency infrastructure, networking, wire representation, native state, and measured hot paths.

## Repository layout

Product code lives entirely under `modules/`:

```text
modules/
├── php/
│   ├── Server/
│   ├── Session/
│   ├── Event/
│   ├── Command/
│   ├── Task/
│   ├── Native/
│   └── Plugin/
└── rust/
    ├── core/
    ├── codec/
    ├── network/
    ├── session/
    └── php-extension/
```

There is intentionally no product `src/` or `native/` directory at repository root. Rust crates still use their normal local `src/` directories.

Composer maps `Cobblestone\\` directly to `modules/php/`. PHP names describe responsibilities instead of a generic Kernel/Core bucket: the running server is `Cobblestone\Server\Server`, session join ordering lives in `Cobblestone\Session\JoinFlow`, and native session bridge values stay under `Cobblestone\Native\Session`.

Rust crate names remain stable (`cobblestone-core`, `cobblestone-codec`, `cobblestone-network`, `cobblestone-session`, `cobblestone-core-php`) even though their repository paths now live under `modules/rust/`.

## Developer workflow

Composer is the normal project command surface:

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

`composer serve` incrementally builds `modules/rust/php-extension`, generates the authoritative Composer autoloader, loads the platform extension, and starts `bin/cobblestone`.

Native Zend exports remain owned by the Rust extension and are independent from Composer class paths. CI explicitly verifies the exact `protocol84` export names.

## Engineering state

Durable project state lives under `.agent/`; architecture is documented in `docs/architecture/FOUNDATION.md`.

C001-C007 are complete. The fixed 0.15.10 client has joined through the production PHP-owned server/session stack. The current repository-architecture cleanup is reorganizing the source tree without changing that fixed-target behavior before C008 world work begins.
