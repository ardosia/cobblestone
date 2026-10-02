# Ardosia world API parity

This record tracks semantic parity with the public world substrate from:

- repository: `ardosia/ardosia`
- revision: `766f2a2a073889583334758b500b7b6e05acb1f1`
- oracle: `crates/world`

The goal is one-for-one behavior where the PHP ownership model permits it, not a mechanical Rust syntax port.

## Parity matrix

| Ardosia world export | Cobblestone surface | Parity |
| --- | --- | --- |
| fixed `CHUNK_EDGE`, `SECTION_EDGE`, `SECTION_COUNT`, `WORLD_HEIGHT`, max Y | `WorldBounds` | semantic parity |
| `BlockPos`, `ChunkPos` | same concepts plus `BlockPos::sectionY/localY` | semantic parity |
| `SectionY` | `SectionY` | semantic parity |
| `Section` | `ChunkSection` | semantic parity; Cobblestone also stores protocol-era nibble/light planes |
| `ChunkData` | native `WorldStore` chunk data behind the PHP `Chunk` facade | semantic parity; Cobblestone also retains height/extra-data/light planes required by the fixed target |
| `ChunkRevision` | `ChunkRevision` + `Chunk::terrainRevision()` | semantic parity |
| `ChunkTerrain` | `ChunkTerrain` facade over one resident `Chunk` | semantic parity |
| `TerrainPatch` | `TerrainPatch` | semantic parity |
| `PreparedTerrainPatch` | `PreparedTerrainPatch` | semantic parity |
| `TerrainEdit` / boolean commit result | same names/concepts | semantic parity |
| `LightLevel` | `LightLevel` at ergonomic/snapshot edges; scalar `int` levels on propagation hot paths | semantic parity with a deliberate allocation-free hot-path adaptation |
| `LightRevision` | `LightRevision` + independent chunk light revision | semantic parity |
| `ChunkLight` | `ChunkLight` facade over chunk light channels | semantic parity |
| `LightEdit` / boolean commit result | same names/concepts | semantic parity |
| `LightSnapshot` | `LightSnapshot` | semantic parity |
| `LightLayer` | `LightLayer` | semantic parity |
| `LightUpdate` | `LightUpdate` | semantic parity |
| `LightAccess` | scalar state/light `LightAccess` over revision-pinned chunk snapshots | semantic behavior parity; PHP hot path avoids per-cell native calls/value allocations |
| `apply_light_update` | `world-light/LightPropagator::apply()` | algorithm/order parity with the pinned Ardosia implementation |
| binary-derived block light properties | `world-light/BlockLightCatalog` | mapped from Ardosia dense semantic identities to fixed-target legacy IDs using the pinned 0.15.10 BlockIds vocabulary |
| `ResidentChunkCell` / `ChunkLease` | shared owner-runtime cell + lifetime pin mirrored into native `WorldStore` | semantic resident identity + snapshot parity; safe unload cannot invalidate a live lease |
| terrain/light lock guards | direct owner-runtime access | deliberate runtime adaptation; no PHP lock ceremony |
| `ChunkSnapshot` | scalar-first `ChunkSnapshot` + ergonomic `terrain()` / `light()` views | semantic parity plus fixed-target protocol projection fields |

## Fixed-target light mapping

Ardosia's catalog uses 191 dense semantic block ordinals. Cobblestone's public `BlockState` intentionally uses the fixed-target legacy block ID plus 4-bit data because that is the native protocol-84 representation.

For lighting, the recovered Ardosia light-block/emission properties are mapped onto legacy IDs using:

- `KhronosDevs/PocketMine-MP@15272732371b4e7785cc9f45b6274b31198d518e`
- `legacy/old-src/block/BlockIds.php`

This yields the same 191 supported fixed-target block identities while retaining Cobblestone's protocol-native state token. Unknown/unmapped legacy IDs are not guessed: the light catalog returns unsupported.

## Deliberate adaptations

Rust's `TerrainReadGuard`, `TerrainWriteGuard`, `LightReadGuard`, and `LightWriteGuard` exist because Ardosia's resident terrain/light cells are shared behind Rust locks. Cobblestone's architecture requires one authoritative PHP runtime owner for mutable gameplay state, so exposing lock guards to ordinary PHP would violate the project API invariant. Direct owner-local terrain/light facades replace those guard types.

Cobblestone does not expose mutable section objects through a `section_mut` equivalent because direct section mutation could bypass height-map and revision invariants. Equivalent block/terrain edit operations are available through `Chunk`, `ChunkTerrain`, and `TerrainEdit`.

Cobblestone retains height map, sparse extra block data, generated/populated/light-populated flags, and protocol-friendly snapshot planes. These are fixed-target requirements beyond the minimal Ardosia `crates/world` terrain model and do not weaken parity.

## Validation boundary

Parity is not considered verified until the relevant Cobblestone revision passes canonical local `composer verify`, including native-world, world parity, mutation, and light smokes.
