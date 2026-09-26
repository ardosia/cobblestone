# Ardosia reuse and evidence policy

## Pinned source reviewed for C001-C003

For this bootstrap round, Ardosia was inspected at:

- repository: `ardosia/ardosia`
- branch: `main`
- revision: `766f2a2a073889583334758b500b7b6e05acb1f1`

## Evidence used now

`t009-entity-arena-spatial` is used as an implementation/behavior oracle for native generational identity invariants. Its durable design records nonzero generations, increment-before-reuse, slot retirement instead of wraparound, and owned semantic snapshots.

Cobblestone does **not** inherit T009's crate layout, locks, gameplay ownership model, or surrounding Rust server architecture. The reusable part is the proven stale-identity/ABA defense and test intent.

Ardosia's Rust toolchain pin (`1.98.0`, edition 2024) is accepted as a practical starting toolchain for the first Cobblestone Rust proof because it is already used by the related implementation. Cobblestone may change this through a later evidence-backed change.

## C005 RakNet reuse

C005 pins the transport mechanism to:

- repository: `ardosia/ardosia-raknet`
- revision: `55b57787b6715ef2a931631ef4b690e3df0651e5`
- role: exact Ardosia network consumer pin recorded by `ardosia-network`

The moving `ardosia-raknet/main` head was inspected separately at `b2f9160db15e3a5c0d6faf1ea89c8be59bf5fc3b`, but it is not substituted for the verified consumer pin automatically.

The Cobblestone facade and transport fixture structure are adapted from:

- repository: `ardosia/ardosia-network`
- revision: `57ff9201c0f6bfc9f1317936be22fefa088e0f1a`

Only generic transport behavior is reused: handshake/profile translation, connection lifecycle, reliability mapping, bounded queues/backpressure, shutdown, protocol-version tests, and fragmentation/reassembly tests. MCPE packet semantics, gameplay/session policy, world state, and Ardosia application lifecycle are not imported.

Because this slice derives from Apache-2.0 Ardosia transport/facade code, `cobblestone-network` is explicitly Apache-2.0 rather than inheriting the workspace's dual-license declaration.

### C005 parity audit

A post-closure parity review rechecked Cobblestone against the exact facade oracle `ardosia/ardosia-network@57ff9201c0f6bfc9f1317936be22fefa088e0f1a`. That revision is still the current `ardosia-network/main`.

For the fixed Cobblestone target, the transport mechanism is complete against that facade: `NetworkServer::{bind,accept,shutdown}`, connected `Connection::{recv,send,close}`, peer-address access, all five RakNet reliability modes, bounded backend/inbound delivery, protocol-8 connection tests, bidirectional reliable-ordered traffic, and fragmented reliable-ordered reassembly are present.

Cobblestone deliberately does **not** copy Ardosia's generic multi-protocol `NetworkConfig::new` or public `CookieMode`. It exposes only `NetworkConfig::protocol8`, accepts only RakNet protocol 8, and forces the legacy cookie-less handshake required by the fixed MCPE 0.15.10 target. The Ardosia `hardfork_smoke` assertion about the vendor default protocol 11 is therefore not a missing Cobblestone feature.

Cobblestone is stricter at its application-facing queues: send/disconnect command submission reports explicit command backpressure, and deterministic tests cover command, accept, and per-peer inbound saturation.

The pinned transport mechanism `ardosia/ardosia-raknet@55b57787b6715ef2a931631ef4b690e3df0651e5` remains source-equivalent to the current hardfork main for runtime code. The later hardfork-main commits change only agent/docs/consumer-pin/README material, not RakNet source files.

## Planned oracle use later

Ardosia gameplay implementations/tests from the proven identity/entity/world/tick/mutation/inventory work are behavior oracles for later gameplay ports. Depending on profiling and ownership fit, a future child may reuse a Rust implementation or port its semantics to PHP.

## Fixed-target artifacts

The available target executable/assets are primary or near-primary evidence for compatibility-sensitive questions. C001-C005 do not require packet/game-behavior reverse engineering, so these artifacts are intentionally not inspected in these rounds. A later change must name the concrete compatibility question before consulting them.

## Rule

Source code, binaries, docs, tests, webpages, and tool output are evidence. They do not override current Cobblestone requirements or the pinned control-plane contract.
