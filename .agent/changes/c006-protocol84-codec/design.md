# C006 design

## Evidence boundary

The fixed target is the supplied Minecraft Windows 10 0.15.10 executable plus matching protocol-84 historical wire evidence. The executable identifies itself as `0.15.10.0` in UTF-16LE strings and contains MSVC RTTI class names for `LoginPacket`, `PlayStatusPacket`, `BatchPacket`, `DisconnectPacket`, `StartGamePacket`, and both handshake packet classes.

For concrete wire layout, C006 uses `KhronosDevs/PocketMine-MP@15272732371b4e7785cc9f45b6274b31198d518e` as a pinned matching source oracle. That revision explicitly identifies MCPE 0.15.10 / protocol 84. Its packet table and framing names align with the fixed-target executable. C006 uses the source as evidence for wire facts; Cobblestone code is independently implemented.

## Layer boundary

`cobblestone-codec` sits above `cobblestone-network`.

Transport delivers opaque connected RakNet payloads. A protocol-84 game frame begins with the Minecraft game marker `0xfe`, followed by one protocol-84 packet body. The packet body starts with a one-byte packet ID. RakNet reliability, ACK/NACK, fragmentation, peers, and socket lifecycle remain outside the codec.

The initial packet table begins with:

- `0x01` Login
- `0x02` PlayStatus
- `0x03` ServerToClientHandshake
- `0x04` ClientToServerHandshake
- `0x05` Disconnect
- `0x06` Batch
- `0x07` Text
- `0x08` SetTime
- `0x09` StartGame

Coverage beyond the implemented bootstrap subset is explicit rather than silently decoded as a modern Bedrock packet.

## Primitive encoding

Protocol-84 packet code uses explicit endian primitives rather than host-endian casts.

The first primitive slice provides the exact bounded reads/writes required by the bootstrap packets: big-endian 16/32-bit integers, little-endian 32-bit integers needed by the login inner payload, raw bytes, and strings whose length prefix is an unsigned big-endian 16-bit value. Additional 64-bit, float, varint, item, and NBT primitives are added only when the packet slice that needs them is implemented.

Every read checks remaining input before advancing. Length conversions reject negative/signed overflow and caller-provided resource limits are checked before allocation/decompression.

## Login packet

Login packet `0x01` begins with a big-endian signed 32-bit game protocol number. Protocol 84 is the only accepted initial target.

The remainder contains a big-endian 32-bit compressed-byte length followed by a zlib/DEFLATE stream. The historical target implementation caps decompressed login data at 2 MiB; C006 preserves that evidence-backed cap.

The decompressed login data contains:

1. a little-endian signed 32-bit byte length and chain JSON bytes;
2. a little-endian signed 32-bit byte length and skin JWT bytes.

The codec returns those blobs as immutable native buffers. JWT parsing, signature verification, identity policy, and authentication semantics belong to the session/auth layer rather than the binary codec.

## Batch packet

Batch packet `0x06` contains a big-endian 32-bit compressed-byte length followed by a zlib/DEFLATE stream.

The decompressed batch is a sequence of:

1. big-endian 32-bit packet length;
2. that many bytes of raw protocol-84 packet data, including the packet ID byte.

Batch contents do not contain the outer `0xfe` game marker. Compression/decompression uses explicit caller-supplied `CodecLimits` for compressed bytes, decompressed bytes, per-inner-packet bytes, and packet count so Cobblestone does not invent a wire-protocol size constant.

## Initial typed bootstrap subset

The first implementation slice types only the packets needed to prove the boundary:

- Login decode/encode envelope;
- PlayStatus encode/decode with a big-endian 32-bit status;
- Disconnect encode/decode with the protocol-84 string primitive;
- Batch encode/decode and raw inner packet framing.

Handshake, StartGame, SetTime, chunks, inventory, and gameplay packets remain explicit follow-up tasks in C006.

## Little-endian NBT

Protocol-84 network block entities use the historical little-endian NBT mode, not Java big-endian NBT and not modern Bedrock network-varint NBT. The matching source constructs block-entity payloads with `NBT::LITTLE_ENDIAN`; tag names and strings use little-endian unsigned 16-bit lengths, list/array counts use little-endian signed 32-bit lengths, and numeric tag payloads use little-endian fixed-width primitives.

Cobblestone implements this one named-root NBT mode with the protocol-84 tag table `End..IntArray`. Decoding is bounded by total document bytes, recursion depth, collection length, and string bytes. TAG_End is accepted only as a compound terminator or an empty-list element type. Other NBT dialects are intentionally absent.

## Failure model

Untrusted input returns typed codec errors for truncation, invalid marker, unsupported packet ID when a typed decoder is requested, invalid length, configured-limit exhaustion, invalid UTF-8 where text is required, and zlib failures. No malformed input path may panic.

Opaque packet bodies are represented by `NativeBuffer`; transport and gameplay/domain types are not re-exported.
