# C005 design

## Proven source

The transport mechanism is pinned to `ardosia/ardosia-raknet@55b57787b6715ef2a931631ef4b690e3df0651e5`, the exact revision recorded as the active Ardosia network consumer pin. The moving hardfork `main` branch is evidence only and is not substituted silently.

Facade structure and integration tests are adapted from `ardosia/ardosia-network@57ff9201c0f6bfc9f1317936be22fefa088e0f1a`. That facade already demonstrates a game-agnostic boundary over the pinned hardfork. The adapted Cobblestone crate is Apache-2.0 because this slice derives from Apache-2.0 Ardosia transport/facade code.

## Boundary

`cobblestone-network` owns UDP/RakNet listener lifecycle, connected peers, reliability selection, bounded command delivery, bounded per-peer inbound payload queues, transport shutdown, and transport error conversion.

It does not own MCPE protocol-84 packets, batch/compression, NBT, gameplay sessions, players, worlds, plugins, or server lifecycle policy.

## Fixed target profile

The public configuration constructor is explicitly `NetworkConfig::protocol8`. It translates to vendor transport configuration with only protocol 8 accepted and handshake cookies disabled. The advertisement remains an opaque string supplied by the layer above.

No generic protocol list or cookie toggle is exposed yet. This keeps the implementation honest to the accepted fixed target instead of generalizing for modern Bedrock prematurely.

## Backpressure

Backend commands use a bounded queue. Each connected peer receives a bounded inbound queue. If a peer's inbound queue fills, the transport disconnects that peer and surfaces a backpressure terminal state instead of accumulating unbounded payloads.

## Verification

Integration tests use the pinned hardfork itself as a protocol peer. They require raw protocol-8 Request1 acceptance, protocol-11 incompatibility rejection, a completed protocol-8 connection, and reassembly of a 4096-byte reliable-ordered payload that must fragment at the RakNet layer.

These tests validate generic RakNet mechanics only. Exact MCPE protocol-84 compatibility remains C006 and later end-to-end fixture work.
