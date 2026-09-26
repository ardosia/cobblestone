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

Backend commands use a bounded queue. Connection send/disconnect requests use nonblocking submission: when that queue is full, callers receive `NetworkError::CommandBackpressure` instead of accumulating an unbounded waiter backlog.

Each connected peer receives a bounded inbound queue. If a peer's inbound queue fills, the transport disconnects that peer and surfaces an inbound-backpressure terminal state instead of accumulating unbounded payloads.

Listener acceptance also uses a bounded queue sized from the configured connection capacity. These mechanisms keep all application-facing transport crossings bounded while making the saturation behavior explicit.

## Verification

Integration tests use the pinned hardfork itself as a protocol peer. They require raw protocol-8 Request1 acceptance, protocol-11 incompatibility rejection, a completed protocol-8 connection, bidirectional reliable-ordered payload flow, and reassembly of a 4096-byte reliable-ordered payload that must fragment at the RakNet layer.

Focused unit tests force accept-queue, command-queue, and per-peer inbound saturation so every application-facing bounded-queue backpressure branch is exercised deterministically rather than relying on timing-sensitive load.

These tests validate generic RakNet mechanics only. Exact MCPE protocol-84 compatibility remains C006 and later end-to-end fixture work.
