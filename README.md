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

Canonical validation entrypoint:

```text
python tools/ci.py all
```

C001 establishes engineering state. C002 proves `cobblestone-core` primitives and the PHP/native boundary. C003 is a multi-runtime torture prototype and is a go/no-go gate before gameplay is distributed across PHP runtimes.

C005 introduces `cobblestone-network`, a game-agnostic RakNet transport facade pinned to the exact Ardosia transport revision recorded in project provenance. Its initial public configuration is intentionally fixed to RakNet protocol 8 with the legacy cookie-less handshake. Protocol-84 packet encoding/decoding remains C006 work.
