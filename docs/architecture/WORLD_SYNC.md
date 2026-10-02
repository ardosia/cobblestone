# Live world synchronization

This document defines the production boundary for synchronizing authoritative world changes to fixed-target MCPE/Win10 0.15.10 clients.

## Ownership boundary

PHP owns gameplay semantics: plugins, block behavior, mutation callbacks, light rules, and the decision to change world state. Rust owns the physical native WorldStore, change capture, coalescing, protocol-84 encoding, compression/cache reuse, viewer routing, queue backpressure, and network fan-out.

A gameplay mutation must not marshal a list of changed blocks back through PHP merely so PHP can send them to clients. Successful native patch application records the resulting semantic change in a bounded native journal. Server tick performs one PHP-to-native flush call for the world.

## Fixed-target packet evidence

The pinned 0.15.10 / protocol-84 source oracle defines UpdateBlock as packet 0x13 with:

- signed big-endian 32-bit X;
- signed big-endian 32-bit Z;
- unsigned byte Y;
- unsigned byte block ID;
- one byte containing four flag bits in the high nibble and legacy block data in the low nibble.

Authoritative block broadcasts in the matching source use FLAG_ALL_PRIORITY, equal to NEIGHBORS | NETWORK | PRIORITY (0x0b). The supplied Win10 executable independently contains RTTI vocabulary for UpdateBlockPacket, RemoveBlockPacket, and FullChunkDataPacket.

## Native change journal

WorldStore maintains an 8,192-entry sequenced ring. Generation/fill initialization does not journal because no client can observe those incomplete states. Atomic semantic patch commits journal only after the patch succeeds.

A journal entry is either:

- Blocks: up to 256 final scalar state IDs keyed by chunk-local linear block index; or
- FullChunk: the chunk must be projected from its latest immutable snapshot.

Light changes, biome changes, block-extra-data changes, and large terrain changes use FullChunk. Point block changes remain Blocks.

The journal is a delivery/recovery mechanism, not persistent storage and not an event API.

## Viewer cursors and backpressure

After native initial chunk streaming completes, the session records a native WorldView containing world handle, center chunk, effective radius, and the exact current world-change sequence.

Each flush:

1. snapshots the journal once;
2. filters changes by each viewer's current view;
3. coalesces repeated writes to the same block to the latest state;
4. promotes a chunk to FullChunk when required;
5. encodes the result natively;
6. queues bounded reliable-ordered Batch packets; and
7. advances the viewer cursor only after every Batch for that flush was accepted.

A full session command queue is not treated as success. The cursor remains unchanged and the authoritative updates are retried on a later tick. If a viewer falls behind the bounded journal, native recovery resends the viewer's complete current chunk view and then advances the cursor.

Disconnected/stale sessions are removed from the native viewer registry. Journal entries are pruned only through the minimum cursor of remaining viewers.

## Packet selection

The fixed codec allows at most 256 packets inside one Batch. The live-sync path therefore caps a Batch at 256 inner packets.

For one chunk:

- 1..256 coalesced block-state-only changes use UpdateBlock;
- more than 256 block-state changes use FullChunkData;
- any light, biome, or block-extra-data change uses FullChunkData.

Changes spanning multiple chunks may produce multiple bounded Batch packets in one flush.

This rule is intentionally conservative for correctness. Future protocol-specific light/block-entity packets may replace some FullChunkData fallbacks when the relevant fixed-target semantics are implemented.

## Measured boundary

Measurements on the Fedora development machine, PHP 8.5.11 ZTS and Rust 1.98 release builds:

| Operation | Measured cost |
| --- | ---: |
| empty-ish PHP/native runtime-id crossing | ~147 ns |
| native terrain revision through FFI | ~362 ns |
| direct native block-state read through FFI | ~463 ns |
| PHP `Native\World` block-state adapter | ~577 ns |
| full native chunk snapshot projection | ~105 us |
| complete PHP Chunk::snapshot() on native chunk | ~115 us |
| PHP-local scalar snapshot state read | ~417 ns |
| idle world-sync flush FFI call | ~292 ns |
| native point patch including journal append | ~450 ns |
| native current journal sequence read | ~24 ns |
| clone 256-entry journal snapshot | ~21.4 us |
| one PHP semantic block mutation | ~23.7 us |
| 256 edits in one semantic mutation | ~7.56 us/block |
| 256 separate semantic mutations | ~23.24 us/block |
| 768 edits in one semantic mutation | ~7.09 us/block |
| 1024 edits in one semantic mutation | ~7.00 us/block |

Native snapshot capture is therefore not a universal replacement for point reads. Staged mutation switches to a snapshot only during prepare when at least 768 authoritative comparisons are pending. Lighting remains snapshot-based because propagation repeatedly scans the same chunk columns and neighbors.

Release codec measurements for one compressed protocol-84 Batch:

| Payload | Encoded bytes | Encode/compress |
| --- | ---: | ---: |
| 1 UpdateBlock | 22 | ~96 us |
| 32 UpdateBlocks | 187 | ~163 us |
| 64 UpdateBlocks | 316 | ~217 us |
| 128 UpdateBlocks | 544 | ~286 us |
| 256 UpdateBlocks | 977 | ~425 us |
| default Flat FullChunkData | 149 | ~401 us |
| high-entropy FullChunkData | 82,272 | ~3.70 ms |

The 256-update escalation point matches the codec packet-count ceiling and avoids pathological full-chunk resends for high-entropy chunks.

## Verification

The canonical integration smoke uses a real protocol-8 RakNet client. It completes protocol-84 login/chunk-radius spawn, PHP mutates block (128,5,128), and the client must receive an UpdateBlock 0x13 for stone with FLAG_ALL_PRIORITY. This exercises PHP semantics, native WorldStore journaling, cursor/coalescing, Batch encoding, session queues, RakNet delivery, and client-side decoding as one path.

Movement-driven view changes are separate follow-up work. The current viewer center/radius is the spawn-time view established by the existing chunk-radius flow.
