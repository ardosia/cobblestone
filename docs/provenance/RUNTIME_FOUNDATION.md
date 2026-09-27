# Runtime/logging/mutation provenance

## Spring Boot logging layout

The visual console layout is derived from the official Spring Boot logging reference reviewed on 2026-09-27:

- https://docs.spring.io/spring-boot/reference/features/logging.html

The relevant visual fields are millisecond timestamp, level, process ID, `---`, application name, thread/execution label, logger name, and message. Cobblestone adapts those fields to PHP/Monolog and omits Spring Boot's banner/configuration system.

PSR-3 remains the API boundary; Monolog is the default implementation.

## Ardosia mutation and region evidence

Pinned oracle:

- repository: `ardosia/ardosia`
- revision: `766f2a2a073889583334758b500b7b6e05acb1f1`

Reviewed surfaces:

- `crates/world/src/edit.rs` — staged terrain patches, base revisions, prepare/commit, no-op detection;
- `crates/world/src/coord.rs` — fixed-target chunk/coordinate semantics;
- `crates/game/src/world_root.rs` — authoritative world residency boundary;

Cobblestone ports the semantic transaction properties to PHP. It does not copy Ardosia's Rust world state, lock graph, leases, or gameplay implementation.

## Region scope

Execution regions are Cobblestone infrastructure, not a Minecraft 0.15.10 world-format claim. The initial 8×8 chunk mapping is tunable and intentionally distinct from disk region-file concepts.

PHP determines semantic region membership. Rust core stores owner/epoch routing records. The current production server still has one owning PHP gameplay runtime; this change does not claim region-parallel gameplay execution.
