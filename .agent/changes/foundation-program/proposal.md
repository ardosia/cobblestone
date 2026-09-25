# Cobblestone foundation program

Build Cobblestone as a fixed-target PHP-first server with Rust/native mechanisms. The program is intentionally decomposed so the PHP/native and concurrency foundations are proven before gameplay is ported at scale.

C001 bootstraps durable engineering state. C002 proves `cobblestone-core` primitives and the PHP 8.5 ZTS extension boundary. C003 tortures the multi-runtime hypothesis before any production gameplay architecture depends on it.

Later children introduce ownership, RakNet, protocol-84 codec, PHP server kernel, native world support where justified, proven gameplay semantics, instrumentation, multi-runtime gameplay ownership, worldgen/storage, and compatibility/performance convergence.
