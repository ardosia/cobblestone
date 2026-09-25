# Cobblestone foundation requirements

## CB-001 — Fixed compatibility target
Cobblestone initially targets Minecraft Windows 10 Edition Beta / MCPE 0.15.10, game protocol 84, and RakNet protocol 8. Compatibility-sensitive behavior requires target-specific evidence and tests.

## CB-002 — PHP owns gameplay semantics
PHP is the primary high-level language for gameplay, events, commands, plugins, and developer-facing APIs. Native code must not turn PHP into a thin guest scripting layer over a monolithic engine.

## CB-003 — Native code owns mechanisms
Rust/native components may own networking, bounded worker infrastructure, native representations, immutable buffers/snapshots, codecs, storage mechanisms, and measured hot paths while preserving semantic boundaries.

## CB-004 — Single-owner mutable authority
Each mutable authoritative game object has exactly one owning PHP runtime at a time. Arbitrary mutable Zend objects are never made globally shared and thread-safe.

## CB-005 — Stable generational identity
Long-lived native-backed identities use type-safe generational handles. Stale handles must be rejected safely; generation exhaustion must not wrap into an ABA hazard.

## CB-006 — Ownership gates mutation
A native handle identifies an object but does not grant mutation rights. Wrong-owner mutations must either route a semantic command to the owner or fail safely when implicit routing would violate API semantics.

## CB-007 — Workers do not call arbitrary Zend APIs
Native background workers perform mechanism work and return completions/messages. Only the owning PHP runtime observes completions and runs arbitrary PHP/Zend code.

## CB-008 — Bounded concurrency boundaries
Cross-thread and cross-runtime queues are bounded and define backpressure, cancellation, shutdown, and stale-message behavior. No world/game lock may be held across I/O.

## CB-009 — Multi-runtime gameplay is gated
The single-runtime PHP server plus native workers must be correct and measured before gameplay is distributed across multiple persistent PHP runtimes. C003 is an explicit torture-test gate, not a production architecture declaration.

## CB-010 — Threading stays out of normal plugin APIs
Ordinary plugin/gameplay code must not require runtime IDs, region execution calls, mutexes, thread synchronization, native pointer concepts, or handle resolution ceremony.

## CB-011 — Transport/wire/domain boundaries stay distinct
RakNet transport, protocol-84 wire packets, storage encodings, network time, and gameplay/domain APIs remain distinct ownership layers.

## CB-012 — Observability precedes tuning
Cobblestone must instrument tick/work latency, queue latency, packet throughput/latency, cross-runtime messages, FFI calls, copied bytes, PHP/native memory, GC behavior, worker utilization, CPU/core use, and network-shard utilization before performance or GC policy claims are accepted.

## CB-013 — GC remains enabled by default
PHP cyclic GC is not globally disabled by architecture. Scheduled/manual collection may be introduced only after measurements demonstrate a meaningful latency problem and the alternative is validated.
