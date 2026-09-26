# C005 proposal

Create a dedicated `cobblestone-network` crate by reusing the proven generic Ardosia RakNet transport rather than rebuilding RakNet mechanics from scratch.

The transport dependency is pinned to the exact Ardosia consumer revision `55b57787b6715ef2a931631ef4b690e3df0651e5`. Cobblestone wraps that transport behind its own narrow facade so later server code does not depend on vendor layouts or expose RakNet internals as gameplay APIs.

The initial compatibility surface is intentionally fixed: RakNet protocol 8, legacy cookie-less handshake, bounded queues, opaque connected payloads, and explicit reliability choices. Protocol-84 packet semantics remain C006 work.
