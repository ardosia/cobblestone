# C006 proposal

C006 introduces `cobblestone-codec` as the fixed-target Minecraft game-protocol wire boundary above `cobblestone-network`. The initial target is exactly Minecraft Windows 10 Edition Beta / MCPE 0.15.10 game protocol 84.

The crate owns binary packet primitives, the protocol-84 packet envelope/dispatch table needed by the initial session bootstrap, batch/compression handling, and NBT only where the protocol actually requires it. It consumes and produces immutable byte buffers. It does not own RakNet reliability/session state, players/worlds/plugins, runtime ownership, or domain semantics.

Compatibility-sensitive details are evidence-driven. C006 may inspect the supplied fixed-target executable/assets because this active change specifically needs protocol-84 wire evidence. Matching historical Ardosia/source material may be used as a pinned oracle when its target identity is established, but modern Bedrock behavior is not substituted implicitly.

Implementation starts with the smallest login/session bootstrap subset needed for a real protocol-84 connection. Packet coverage expands only when an accepted higher-level slice needs it.
