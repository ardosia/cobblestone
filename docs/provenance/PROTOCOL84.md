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
