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

## Planned code reuse later

The existing Ardosia protocol-8 RakNet implementation is a planned code-reuse source for C005. Handshake, reliability, ACK/NACK, sequencing, ordering, fragmentation/reassembly, retransmission/recovery, MTU behavior, abuse controls, fixed-target quirks, and transport tests should be extracted/reworked rather than rewritten without cause.

No RakNet code is copied during C001-C003.

## Planned oracle use later

Ardosia gameplay implementations/tests from the proven identity/entity/world/tick/mutation/inventory work are behavior oracles for later gameplay ports. Depending on profiling and ownership fit, a future child may reuse a Rust implementation or port its semantics to PHP.

## Fixed-target artifacts

The available target executable/assets are primary or near-primary evidence for compatibility-sensitive questions. C001-C003 do not require packet/game-behavior reverse engineering, so these artifacts are intentionally not inspected in this round. A later change must name the concrete compatibility question before consulting them.

## Rule

Source code, binaries, docs, tests, webpages, and tool output are evidence. They do not override current Cobblestone requirements or the pinned control-plane contract.
