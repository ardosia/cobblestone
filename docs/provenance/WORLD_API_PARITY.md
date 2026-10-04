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
| fixed-target dimension identity | backed `Dimension` enum + native/storage/protocol projection | exact live 0.15.10 domain: Overworld/Normal `0`, Nether/Hell `1`; persisted and projected through StartGame |
| fixed-target Overworld biome source | `Generator\\OverworldBiomeSource` + native layered source + immutable `BiomeArea` | recovered 0.15.10 layer graph; deterministic seed/coordinate parity cross-checked byte-for-byte against an independent Java-1.8 layer oracle |
| Infinite Overworld pre-surface terrain shape | native `OverworldTerrainShape` → immutable `ChunkTerrainShape` | exact recovered `RandomLevelSource::prepareHeights`: MT/Perlin density, raw 1:4 biomes, sea-level fill, 16×128×16 coarse projection |
| Infinite Overworld surface building | native `OverworldSurfaceBuilder` → immutable `SurfacedChunk` | exact recovered `RandomLevelSource::buildSurfaces`: chunk MT reseed, surface simplex, bedrock, final-biome top/filler + specialized Mesa/Swamp/Taiga/Hills/mutated rules |
| Infinite Overworld cave carving | native `OverworldCaveCarver` → immutable `CarvedChunk` | exact recovered `LargeCaveFeature`: radius-8 source scan, MT reseed, room/tunnel recursion, water abort, lava cutoff, target diggable/repair rules |
| Infinite Overworld lake population | native mutable `PopulationNeighborhood` + `OverworldLakePopulator` | exact recovered pre-structure population reseed/gates plus `LakeFeature` cavity validation, water/lava writes, lava boundary stone conversion, cross-chunk writes |
| Infinite Overworld ore decoration | native mutable `PopulationNeighborhood` + `OverworldOreDecorator` | exact recovered 3x3 population boundary, biome-decoration reseed, common PE ore sequence/geometry, extra-gravel burst, Mesa extra gold, cross-chunk writes |
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

## Fixed-target Overworld biome-source parity

The uploaded 0.15.10 executable establishes the GenLayer-style source family and concrete stage graph: island/fuzzy zoom, repeated land expansion, ocean reduction, snow/climate edges, mushroom/deep-ocean, biome initialization/edges/hills/rare mutation, shore, river branch/mix, smooth, and final Voronoi zoom. The recovered graph preserves the original integer-wrap RNG constants and stage seeds, the 32-bit MCPE world-seed input widened as raw unsigned bits into the 64-bit layer RNG, and the pre-1.13 Hills auxiliary-zoom zero-initialization quirk.

`native/world::OverworldBiomeSource` owns that hot mechanism. PHP exposes a typed `Generator\\BiomeSource` / `OverworldBiomeSource` surface and receives one compact `BiomeArea` byte plane per coarse sample rather than crossing FFI once per column or allocating one object per cell. `BiomeArea::columnAt()` applies the catalog default color only when a semantic generated column is requested. Existing stored `BiomeColumn` words remain authoritative after generation/mutation.

Independent validation uses `Cubitect/cubiomes@e61f90580cbdd883214a8054670dacae655e59c0` in `MC_1_8` mode only as a cross-check after the binary established the target graph. Seven hard-coded fixtures match byte-for-byte across positive/negative coordinates, multiple seeds, signed-32-bit seed limits, and odd area sizes. Flat remains preset-driven and intentionally bypasses this Overworld source.

## Fixed-target Infinite terrain-shape parity

The uploaded 0.15.10 executable is authoritative for the `RandomLevelSource::prepareHeights` stage. Cobblestone mirrors its 5×17×5 density lattice, 4×8×4 interpolation order, raw quarter-scale biome sampling, 32-bit-seeded MT19937 stream, exact Perlin bank construction order, biome depth/scale smoothing, and Y<63 still-water fill. Output is an immutable 32,768-state `ChunkTerrainShape` containing only air, stone, and still water.

Five hard-coded pre-surface full-chunk fixtures were produced independently from cubiomes' raw `MC_1_8` biome layer plus a separate target-style MT/Perlin implementation reconstructed from binary-confirmed rules and `theaperturecat/MCPE-1.0.0-Restored@c095fbd72bbd88d25e6a550c86cbcc659924c4e6`. Hashes and air/stone/water counts match for multiple seeds, negative coordinates, signed-32-bit seed limits, and a mutated-biome case.

## Fixed-target Infinite surface-building parity

The target executable is authoritative for the following `RandomLevelSource::buildSurfaces` stage. Cobblestone retains the constructor's four-octave surface simplex instead of merely consuming its RNG initialization, then reseeds one MT stream per chunk with the target wrapping constants `341872712` and `132899541`. Column iteration remains X-major/Z-minor because RNG consumption makes ordering observable. `OverworldSurfaceBuilder` consumes the immutable base shape and final Voronoi biome plane and returns one immutable 32,768-state `SurfacedChunk`; no PHP/per-block FFI path is introduced.

The target biome vtables identify the base surface routine and specialized overrides for Extreme Hills, Mesa, Mutated, Mutated Savanna, Swamp, and Taiga. Cobblestone reproduces their fixed-target surface semantics: default grass/dirt and sand→sandstone runs, cold-water ice substitution, 2..5-layer bedrock, stone beach/mycelium/ice-spikes materials, Extreme Hills gravel/stone thresholds, Mega Taiga podzol/coarse dirt, mutated Savanna thresholds, Swamp's fixed-seed biome-info prepass, and Mesa/Bryce red-sand/hardened/stained-clay bands plus pillar noise. Generic mutated biomes delegate to their parent surface as in the target.

Independent validation first checks the four-octave surface-simplex f32 bit patterns against a standalone C++ implementation of the MCPE MT/Simplex draw sequence. Nine full surfaced chunks are then cross-checked against a separate C++ surface oracle fed only the already parity-locked #24 base states and independent cubiomes `MC_1_8` final-biome bytes. The fixtures cover default/beach, desert/Mesa boundaries, Mesa, Bryce, Swamp, Mega Taiga, mutated Extreme Hills, mutated Savanna, Ice Spikes, negative coordinates, and the `-1` signed seed boundary.

## Fixed-target Infinite cave-carving parity

The 0.15.10 executable exposes `LargeCaveFeature` for Overworld generation and `LargeHellCaveFeature` for Nether generation; no separate Overworld ravine/canyon generator type is present. The recovered `RandomLevelSource` load path applies `LargeCaveFeature` immediately after surface building, making it the complete fixed-target Overworld carving stage for this version.

`OverworldCaveCarver` reproduces the target's world-seed odd X/Z scale derivation, inclusive radius-8 source-chunk scan, wrapping per-source MT reseed, nested cave-count gating, optional room generation, recursive tunnel split, target float trigonometric drift, and `nextGaussianFloat()` draw semantics. Carving preserves the target's unusual raw-buffer behavior: water on the scan boundary is converted to flowing water and aborts that tunnel step; diggable blocks below the geometric Y<10 cutoff become still lava; other diggable blocks become air; thin sand is repaired to sandstone/red sandstone; exposed dirt can become grass; block ID replacement does not clear the metadata nibble.

Independent validation chains two standalone C++ stages rather than consuming Rust output: the already independent #25 surface oracle emits surfaced state bytes, then a separate C++ `LargeCaveFeature` reproduction applies the cave pass. Three cave-bearing full chunks match Rust exactly: seed `0` at `-78:-128` (3064 changed states), seed `0` Mesa at `-58:-128` (1004 changed states), and seed `-1` Ice Spikes at `36:-96` (228 changed states). Every fixture records writes originating from at least one neighboring source chunk, exercising cross-chunk cave continuity.

## Fixed-target Infinite lake-population parity

The target `RandomLevelSource::postProcess` first mutates the 3x3 neighborhood with optional lakes before structure/dungeon work. Infinite passes `false` for the restored source's `legacyDevice` constructor flag, so water placement uses the full target Y range and lava attempts are active. Desert/DesertHills skip the water gate entirely; a selected water attempt sets `hasLake` even if `LakeFeature` later aborts, suppressing the lava attempt. Lava uses the target nested Y distribution and unusual Y gate: 60..63 passes directly, while Y>=64 additionally requires 1/10.

`OverworldLakePopulator` preserves the 16x8x16 union-of-ellipsoids mask, air descent and (-8,-4,-8) origin adjustment, boundary validation, still-liquid/air cavity split, exact lava boundary RNG, and cross-chunk writes. The source's skylight-based grass repair is deliberately inert here because fixed-target generated Overworld chunks retain MIN/zero skylight until the later post-process lighting stage.

Independent validation uses a standalone C++ MCPE-MT/LakeFeature oracle over synthetic 3x3 neighborhoods. Hard fixtures match Rust exactly for a negative-coordinate seed-0 water lake (`0x5e20b4e846e48585`, 20 neighboring states changed), Desert suppression at the same center, failed water placement that still suppresses lava, a cross-chunk Y=60 lava lake over dirt (`0xaa580d1c9aa95d65`, 160 lava + 182 boundary stone states), and signed seed `-1` water placement (`0x5afb2180df11d495`, 122 neighboring states changed).

## Fixed-target Infinite ore-decoration parity

The target population boundary is a 3x3 neighborhood rather than a center-chunk-only write surface. `PopulationNeighborhood` therefore retains nine carved chunk planes as one transient generation-owned unit. `OverworldOreDecorator` accepts that supplied unit instead of manufacturing its own prerequisite state, allowing later lake/structure/dungeon issues to run before biome decoration in the exact target order. The ore stage reseeds MT from the world seed using the target odd X/Z scales and center chunk coordinates immediately before decoration.

The recovered common sequence is dirt 10×33, gravel 8×33, optional PE extra gravel 80×33 below Y50, diorite/granite/andesite 10×33 below Y80, coal 20×17, iron 20×9 below Y64, gold 2×9 below Y32, redstone 8×8 below Y16, diamond 1×8 below Y16, and lapis 1×7 using the target triangular Y distribution around 16. Mesa-family decorators append 20 gold 9-block veins over Y32..79. Every `OreFeature` draw and float/floor ellipsoid operation is preserved, and writes may cross from the center into any required neighbor in the 3x3 boundary.

Independent validation uses a standalone C++ MCPE-MT/OreFeature oracle over synthetic all-stone 3x3 target-format neighborhoods so failures isolate this stage from terrain/cave noise. Five hard fixtures match exactly across negative coordinates and signed seed boundaries. The seed-0 `-1:-16` fixture exercises the PE extra-gravel branch (7333 gravel states, 8072 changed neighbor states); the seed-0 Mesa fixture at `-58:-128` exercises the extra-gold override (137 gold states). All fixtures produce neighboring-chunk writes, and a separate integration test confirms real carved neighborhoods retain those cross-chunk mutations.

## Deliberate adaptations

Rust's `TerrainReadGuard`, `TerrainWriteGuard`, `LightReadGuard`, and `LightWriteGuard` exist because Ardosia's resident terrain/light cells are shared behind Rust locks. Cobblestone's architecture requires one authoritative PHP runtime owner for mutable gameplay state, so exposing lock guards to ordinary PHP would violate the project API invariant. Direct owner-local terrain/light facades replace those guard types.

Cobblestone does not expose mutable section objects through a `section_mut` equivalent because direct section mutation could bypass height-map and revision invariants. Equivalent block/terrain edit operations are available through `Chunk`, `ChunkTerrain`, and `TerrainEdit`.

Cobblestone retains height map, sparse extra block data, generated/populated/light-populated flags, and protocol-friendly snapshot planes. These are fixed-target requirements beyond the minimal Ardosia `crates/world` terrain model and do not weaken parity.

## Validation boundary

Parity is not considered verified until the relevant Cobblestone revision passes canonical local `composer verify`, including native-world, world parity, mutation, and light smokes.
