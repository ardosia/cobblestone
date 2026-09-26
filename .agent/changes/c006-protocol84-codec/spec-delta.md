# C006 spec delta

C006 introduces no new stable requirement IDs. It implements existing foundation requirements:

- **CB-001**: keep the initial compatibility target fixed to MCPE 0.15.10 / game protocol 84;
- **CB-003**: place binary codecs, compression, immutable buffers, and measured wire hot paths in native mechanism code where appropriate;
- **CB-008**: reject malformed or oversized untrusted payloads with explicit bounds rather than allowing unbounded buffering;
- **CB-011**: keep RakNet transport, protocol-84 wire packets, and gameplay/domain APIs as separate ownership layers.

The protocol-84 codec is not a modern Bedrock compatibility layer. Supporting another game protocol requires a later accepted scope expansion.
