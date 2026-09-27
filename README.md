# Cobblestone

Cobblestone is a fixed-target Minecraft Windows 10 Edition Beta / MCPE 0.15.10 server.

Initial compatibility target:

- game protocol: **84**
- RakNet protocol: **8**
- high-level gameplay/plugin language: **PHP 8.5 ZTS**
- native mechanisms and measured hot paths: **Rust/native extensions**

The architecture deliberately keeps PHP in charge of gameplay semantics and developer-facing APIs. Native code owns mechanisms such as networking, bounded workers, native representations, immutable buffers, and measured hot paths. Arbitrary Zend objects are not shared between runtimes.

## PHP package

The PHP kernel is a Composer/PSR-4 project. Production classes live under `src/Cobblestone/` and use the `Cobblestone\\` namespace.

Prepare the autoloader from the repository root:

```text
composer dump-autoload --classmap-authoritative
```

The native `cobblestone_core_php` extension is built separately; Composer owns PHP class loading, not the Rust/PHP extension build.

## Engineering state

The durable project state lives under `.agent/`. The current foundation architecture is documented in `docs/architecture/FOUNDATION.md`.

Canonical correctness validation:

```text
python tools/ci.py all
```

Native microbenchmarks are explicit measurement work rather than a correctness gate:

```text
python tools/ci.py bench
```

C001-C007 are complete. C007 proved the real 0.15.10 client can join through the production PHP-owned `ServerKernel`, reaching Login acceptance, RequestChunkRadius handling, synthetic chunk delivery, and PLAYER_SPAWN through the RakNet-8/session/codec/native bridge.

The next foundation work starts with world/domain structure and observability rather than adding more kernel ceremony: C008 introduces world/chunk structures only where a native boundary is justified, C009 begins deliberate gameplay semantics, and C010 instruments memory/GC/queues/tick behavior before multi-runtime gameplay work.
