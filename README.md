# Cobblestone

Cobblestone is a fixed-target Minecraft Windows 10 Edition Beta / MCPE 0.15.10 server.

Initial compatibility target:

- game protocol: **84**
- RakNet protocol: **8**
- high-level gameplay/plugin language: **PHP 8.5 ZTS**
- native mechanisms and measured hot paths: **Rust/native extensions**

The architecture deliberately keeps PHP in charge of gameplay semantics and developer-facing APIs. Native code owns mechanisms such as networking, bounded workers, native representations, immutable buffers, and measured hot paths. Arbitrary Zend objects are not shared between runtimes.

## Project layout

Core PHP kernel/runtime infrastructure lives under `src/Cobblestone/` and uses the `Cobblestone\\` namespace.

First-party gameplay/domain features live under `modules/` and use `Cobblestone\\Modules\\`. This is only a Composer/PSR-4 source-layout boundary; it does not register runtime modules and does not affect native PHP extension exports.

Third-party/user-facing extensions remain plugins. Rust/native crates remain under `native/`.

## Composer workflow

Composer is the normal developer command surface:

```text
composer modules
composer build
composer check
composer test
composer verify
composer serve
```

Focused native commands are also available:

```text
composer native:check
composer native:build
composer test:php
```

`composer serve` builds the current native adapter incrementally, prepares the authoritative Composer autoloader, loads the platform extension, and starts `bin/cobblestone`. Server settings currently use `COBBLESTONE_BIND`, `COBBLESTONE_SERVER_NAME`, `COBBLESTONE_MAX_CONNECTIONS`, and `COBBLESTONE_TICK_RATE`.

The native `cobblestone_core_php` extension is built separately from Composer class autoloading. CI explicitly checks the exact native function names, including the `protocol84` exports, so reorganizing PHP modules cannot silently rename the Zend function surface.

## Engineering state

The durable project state lives under `.agent/`. The current foundation architecture is documented in `docs/architecture/FOUNDATION.md`.

C001-C007 are complete. C007 proved the real 0.15.10 client can join through the production PHP-owned `ServerKernel`, reaching Login acceptance, RequestChunkRadius handling, synthetic chunk delivery, and PLAYER_SPAWN through the RakNet-8/session/codec/native bridge.

The next foundation work starts with world/domain structure and observability rather than adding more kernel ceremony: C008 introduces world/chunk structures only where a native boundary is justified, C009 begins deliberate gameplay semantics, and C010 instruments memory/GC/queues/tick behavior before multi-runtime gameplay work.
