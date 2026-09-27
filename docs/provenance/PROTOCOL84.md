# Protocol-84 fixed-target provenance

C006 targets Minecraft Windows 10 Edition Beta / MCPE 0.15.10, game protocol 84.

## Supplied artifacts

- `Minecraft.Win10.DX11.exe` — SHA-256 `d5683d2362c62e76dc9e49f8ad08e1462b6f4a8120fca981eb22955368ed38ab`; PE32+ x86-64; embedded UTF-16LE version `0.15.10.0`.
- `assets-win10.zip` — SHA-256 `25c045e00d8e7f6ffa88592d44dc02c687630c1bf7355bfcbb8c9fd2e1956e6b`; 6661 resource entries.
- `assets.rar` — SHA-256 `b5d2038ca2484f76f8d751b48b126cafcc0865f4053bf44006fb3e16e0589509`.

The executable contains RTTI names for the protocol packet classes used to cross-check source vocabulary. C006 does not infer field layouts from class names alone.

## Matching source oracle

Wire facts are cross-checked against `KhronosDevs/PocketMine-MP@15272732371b4e7785cc9f45b6274b31198d518e`, which explicitly identifies itself as an MCPE 0.15.10 / protocol-84 implementation.

Cobblestone does not copy that implementation. The pinned revision is used to establish protocol numbers, framing, endianness, compression shape, and packet field order, then the Rust codec is independently implemented and validated.

Modern Bedrock protocol behavior is not accepted as a substitute for this fixed-target evidence.

## FullChunkData layered terrain

The real-world stream additionally cross-checks the pinned source's `FullChunkDataPacket`,
Anvil/BaseChunk layered serializer, and the separate LevelDB column serializer.

For `ORDER_LAYERED = 1`, packet `0x34` carries big-endian chunk X and Z, a one-byte
order, a big-endian payload length, and then the historical layered payload:

- 32768 block IDs in layered Y/Z/X order, where each 16x16 horizontal layer is contiguous and the semantic index is `(y << 8) | (z << 4) | x`;
- 16384 packed block-data nibbles in the same Y/Z/X order;
- 16384 packed sky-light nibbles in the same Y/Z/X order;
- 16384 packed block-light nibbles in the same Y/Z/X order;
- 256 height-map bytes in Z/X column order;
- 256 big-endian biome words, with the biome ID in the high byte and 24-bit terrain RGB in the low bytes;
- a little-endian 32-bit sparse extra-data count followed by little-endian 32-bit keys and 16-bit values;
- optional tile NBT bytes after sparse extra data.

The pinned old biome implementation computes Plains (id 1) terrain RGB `0x92bc59` from
its 0.15.10 temperature/rainfall model, yielding wire word `0x0192bc59`.

Cobblestone's PHP `ChunkSnapshot` deliberately remains semantic state in Y/Z/X section order.
That is already the order required by protocol-84 `ORDER_LAYERED = 1`; the native codec validates
and appends those planes directly. The distinct LevelDB X/Z/Y-style column storage belongs to
`ORDER_COLUMNS = 0` and must not be transposed into a packet still labelled layered.
The private PHP/native bulk projection is not a gameplay packet; Rust continues to own
FullChunkData framing, biome-word construction, sparse extra-data endianness, and Batch compression.

The initial production stream keeps the client-requested radius distinct from a server-selected
effective radius capped at three for this slice. Radius three is at most 49 chunks and remains
inside the fixed 4 MiB decompressed Batch budget. The former synthetic all-air spawn-probe export
is retained only for ABI compatibility and is not called by production server composition.
