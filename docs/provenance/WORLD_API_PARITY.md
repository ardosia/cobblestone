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
| fixed-target block identity/state catalog | backed `BlockType` enum + `BlockData` + `BlockState` | exact 191 shipped asset identities as singleton semantic cases; scalar `(id << 4) | data` layout retained below the semantic API |
| binary-derived block light properties | `BlockType::lightProperties()` projected through `world-light/BlockLightCatalog` | same authoritative block identities; scalar adapter only on the propagation boundary |
| `ResidentChunkCell` / `ChunkLease` | shared owner-runtime cell + lifetime pin mirrored into native `WorldStore` | semantic resident identity + snapshot parity; safe unload cannot invalidate a live lease |
| terrain/light lock guards | direct owner-runtime access | deliberate runtime adaptation; no PHP lock ceremony |
| `ChunkSnapshot` | scalar-first `ChunkSnapshot` + ergonomic `terrain()` / `light()` views | semantic parity plus fixed-target protocol projection fields |

## Fixed-target block identity/state parity

Primary identity oracle:

- uploaded `assets-win10.zip` (`25c045e00d8e7f6ffa88592d44dc02c687630c1bf7355bfcbb8c9fd2e1956e6b`);
- `data/resourcepacks/vanilla/blocks.json` hashes to `e7b9445531407856c9a3a493f55cdab57814aa0f3eba0abc47eba3cf9a763dc0` and contains exactly 191 block registry names;
- the newline-delimited ordered name list hashes to `9da4af43358bd40ebaa7f205a337bb6dae70f7918cec180efd73df5065ee3a49`; and
- the uploaded 0.15.10 executable independently exposes representative registry strings including `pistonArmCollision`, `nether_brick_fence`, `glowingobsidian`, `info_update2`, and `reserved6`.

Numeric-ID oracle:

- `KhronosDevs/PocketMine-MP@15272732371b4e7785cc9f45b6274b31198d518e`;
- `legacy/old-src/block/BlockIds.php`.

The asset list is not numerically ordered, so each backed `BlockType` case carries its explicit legacy ID while internal metadata preserves the exact asset name. The reconciliation is one-to-one across all 191 identities. Unsupported legacy holes are rejected by both PHP and Rust. The compact state representation remains `(id << 4) | data`; `BlockData` models the exact four-bit metadata domain, and `BlockState` combines the two semantic values without introducing a behavior-class hierarchy.

## Fixed-target light mapping

Ardosia's catalog uses the same 191 semantic identities as the fixed-target registry but represents them as dense ordinals. Its recovered light-block/emission properties are mapped onto the backed `BlockType` identities above. Static light metadata is cached per enum singleton; `BlockLightCatalog` only adapts a scalar `BlockStateId` to that semantic metadata for propagation.

This preserves Cobblestone's protocol-native state token while making state validity, semantic block identity, and lighting agree on one authoritative closed domain. Unknown/unmapped legacy IDs are never guessed.

## Fixed-target biome-column parity

The Ardosia substrate does not define the legacy MCPE biome-word wire/storage model, so this portion is pinned directly to the fixed target.

Primary oracle:

- uploaded `Minecraft.Win10.DX11.exe`, FileVersion/ProductVersion `0.15.10.0`;
- its biome registration/name table and biome RTTI establish the 60 live IDs used by this target;
- the dormant `TheEndBiome` implementation exists in the binary, but biome ID 9 is absent from the live 0.15.10 registration table and is therefore rejected by `BiomeId`; and
- binary biome overrides confirm fixed-target special colors including Swampland `0x6a7039` and Mesa `0x90814d`.

Chunk-layout oracle:

- `KhronosDevs/PocketMine-MP@15272732371b4e7785cc9f45b6274b31198d518e`;
- `legacy/old-src/level/format/generic/BaseFullChunk.php` stores 256 `BiomeColors` words;
- the high byte is biome ID and the low 24 bits are biome color;
- ID and color setters preserve the other component independently; and
- FullChunkData writes those words big-endian.

Default generation colors use the matching fixed-target grass-color interpolation for the registered biome temperature/rainfall values, with binary-confirmed fixed overrides for swamp and mesa and the historical roofed-forest transform. These defaults are used only when creating a new biome column. Stored RGB is authoritative afterward and is never recomputed by the protocol encoder.

Cobblestone exposes this as `BiomeId` + immutable `BiomeColumn`, stores full `u32` words in both PHP/native chunk backends, persists them in semantic payload v2, migrates old payload-v1 ID bytes explicitly, and projects the stored words unchanged to protocol 84.

## Deliberate adaptations

Rust's `TerrainReadGuard`, `TerrainWriteGuard`, `LightReadGuard`, and `LightWriteGuard` exist because Ardosia's resident terrain/light cells are shared behind Rust locks. Cobblestone's architecture requires one authoritative PHP runtime owner for mutable gameplay state, so exposing lock guards to ordinary PHP would violate the project API invariant. Direct owner-local terrain/light facades replace those guard types.

Cobblestone does not expose mutable section objects through a `section_mut` equivalent because direct section mutation could bypass height-map and revision invariants. Equivalent block/terrain edit operations are available through `Chunk`, `ChunkTerrain`, and `TerrainEdit`.

Cobblestone retains height map, sparse extra block data, generated/populated/light-populated flags, and protocol-friendly snapshot planes. These are fixed-target requirements beyond the minimal Ardosia `crates/world` terrain model and do not weaken parity.

## Validation boundary

Parity is not considered verified until the relevant Cobblestone revision passes canonical local `composer verify`, including native-world, world parity, mutation, and light smokes.
