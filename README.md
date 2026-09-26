# Cobblestone

Cobblestone is a fixed-target Minecraft Windows 10 Edition Beta / MCPE 0.15.10 server.

Initial compatibility target:

- game protocol: **84**
- RakNet protocol: **8**
- high-level gameplay/plugin language: **PHP 8.5 ZTS**
- native mechanisms and measured hot paths: **Rust/native extensions**

The architecture deliberately keeps PHP in charge of gameplay semantics and developer-facing APIs. Native code owns mechanisms such as networking, bounded workers, native representations, immutable buffers, and measured hot paths. Arbitrary Zend objects are not shared between runtimes.

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

C001-C003 are complete. C003 validated the process-isolated persistent-runtime topology as an experiment; its subprocess harness lives under `tests/` and is not a server CLI or plugin API.

C004 productionizes owner/epoch enforcement in `cobblestone-core`. C005 provides the fixed protocol-8 RakNet transport. C006 provides the protocol-84 wire codec, batch/compression, initial session bootstrap packets, and the required little-endian NBT mode. C007 is the next foundation slice: the single-runtime PHP server/session kernel that wires transport and codec into an actual client login flow.
