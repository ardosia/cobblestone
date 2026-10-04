# Fixed-target world model provenance

This record covers the initial PHP `world` package for Minecraft Windows 10 Edition Beta / MCPE 0.15.10.

## Supplied binary/resource evidence

The inspected artifacts are the same fixed-target files recorded for protocol 84:

- `Minecraft.Win10.DX11.exe` — SHA-256 `d5683d2362c62e76dc9e49f8ad08e1462b6f4a8120fca981eb22955368ed38ab`
- `assets-win10.zip` — SHA-256 `25c045e00d8e7f6ffa88592d44dc02c687630c1bf7355bfcbb8c9fd2e1956e6b`
- `assets.rar` — SHA-256 `b5d2038ca2484f76f8d751b48b126cafcc0865f4053bf44006fb3e16e0589509`

The executable exposes fixed-target vocabulary for `Level`, `BlockSource`, `ChunkSource`, `MainChunkSource`, `PlayerChunkSource`, `NetworkChunkSource`, `EmptyChunkSource`, `WorldLimitChunkSource`, `Dimension`, `NormalDimension`, `HellDimension`, `BiomeSource`, `FixedBiomeSource`, and references to `LevelChunk`. It also contains `generator.old`, `generator.infinite`, `generator.flat`, `worldGeneratorType`, and `game_flatworldlayers`.

The shipped English pocket localization independently labels the three fixed-target world types as Old, Infinite, and Flat.

Class/vocabulary strings establish names and ownership seams only. They are not treated as field-layout evidence.

For Overworld biome selection, the executable additionally exposes the concrete layered generator family used by the target: `IslandLayer`, `FuzzyZoomLayer`, `ZoomLayer`, `AddIslandLayer`, `RemoveTooMuchOceanLayer`, `AddSnowLayer`, `AddEdgeLayer`, `AddMushroomIslandLayer`, `AddDeepOceanLayer`, `BiomeInitLayer`, `BiomeEdgeLayer`, `RegionHillsLayer`, `RareBiomeSpotLayer`, `RiverInitLayer`, `RiverLayer`, `SmoothLayer`, `ShoreLayer`, `RiverMixerLayer`, and the Voronoi zoom stage. Recovered constructor flow/seeds match the classic 1.8-era graph (`1`, `2000`, `2001`, `2`, `50`, `70`, `100`, `200`, `1000+`, etc.), including the legacy Hills auxiliary branch whose two zoom layers retain zero-initialized layer RNG state while sharing the normally seeded river-init parent.

For Infinite Overworld terrain shape, the same executable exposes `RandomLevelSource` and its `prepareHeights`/density path. Direct binary inspection pins a 5×17×5 density lattice interpolated as 4×8×4 cells into a 16×128×16 chunk; the raw pre-Voronoi biome plane is sampled at quarter scale with a 10×10 neighborhood. The constructor seeds a 624-word Mersenne-Twister state directly from the target's 32-bit `RandomSeed` (not Java's 48-bit `Random`) and constructs noise banks in the fixed order: 16-octave min limit, 16-octave max limit, 8-octave main, 4-octave surface simplex, 10-octave scale, 16-octave depth, 8-octave forest. Surface/scale/forest are not all read by `prepareHeights`, but their constructors consume the shared RNG stream and therefore remain generation-significant.

The executable also exposes the distinct `RandomLevelSource::buildSurfaces` stage and biome virtual surface overrides. Binary RTTI/vtable/disassembly identifies the base `Biome::buildSurfaceAt` plus specialized target overrides for `ExtremeHillsBiome`, `MesaBiome`, `MutatedBiome`, `MutatedSavannaBiome`, `SwampBiome`, and `TaigaBiome`. The stage reseeds its per-chunk MT stream with wrapping `chunkX * 341872712 + chunkZ * 132899541`, samples the retained four-octave surface simplex at 1/8 block scale, places 2..5 bottom bedrock layers per column, and then applies biome top/filler rules. Mesa/Bryce additionally owns world-seeded clay-band, pillar, and roof simplex noise; generic mutated biomes delegate surface behavior to their contained parent.

For Overworld carving, the same 0.15.10 executable exposes `LargeCaveFeature` and `LargeHellCaveFeature`; no separate Overworld ravine/canyon generator RTTI/vocabulary is present, and the recovered `RandomLevelSource` load path applies `LargeCaveFeature` immediately after surfaces. The matching restored MCPE source at `theaperturecat/MCPE-1.0.0-Restored@c095fbd72bbd88d25e6a550c86cbcc659924c4e6` reconstructs the target mechanism: world-seed-derived odd X/Z scales, an inclusive radius-8 source-chunk scan, per-source MT reseeding, nested cave-count gating, room/tunnel recursion, float trigonometric drift with `nextGaussianFloat() == nextFloat() - nextFloat()`, water-boundary abort/flowing-water conversion, Y<10 still-lava replacement, the exact diggable block set, thin-sand sandstone/red-sandstone repair, and exposed-grass repair. Block IDs and metadata are separate in the target, so carve replacement changes the ID byte without clearing its metadata nibble.

For post-carving population, the target executable exposes RTTI for `LakeFeature`, `BiomeDecorator`, `MesaBiome::Decorator`, and `OreFeature`. The pinned restored source shows that `RandomLevelSource::postProcess` owns a 3x3 chunk neighborhood. Infinite/Overworld constructs `RandomLevelSource(..., false)`, so the misleadingly named `legacyDevice` branch is disabled for this generator: underground water attempts are not suppressed and lava-lake attempts are enabled. Water lakes are excluded only for Desert/DesertHills, take a 1/4 gate, choose X/Z in [3,12) and Y in [0,128), and set `hasLake` even when `LakeFeature::place` aborts. If no water attempt claimed the slot, lava takes a 1/8 gate, uses nested Y sampling, then only places for Y>=60 with Y60..63 unconditional and higher Y requiring another 1/10 gate.

`LakeFeature` descends through air, offsets the cavity by (-8,-4,-8), unions 4..7 random ellipsoids into a 16x8x16 mask, aborts when upper boundary cells touch liquid or lower boundary cells are non-solid/nonmatching liquid, fills the lower half with still water/lava and upper half with air, and for lava probabilistically converts solid boundary cells to stone. Its dirt-to-grass repair checks current skylight >0; generated Overworld chunks have default `Brightness::MIN` skylight before the later post-process light update, so that repair is inert at this stage. Out-of-height block reads resolve to air.

The same post-process later reseeds the MT stream immediately before `biome.decorate`, making common ore RNG independent from earlier lake/structure RNG consumption while still observing whatever blocks those earlier stages changed. `BiomeDecorator::decorateOres` places dirt/gravel, the PE-specific 1/16 eighty-vein extra-gravel burst, diorite/granite/andesite, coal/iron/gold/redstone/diamond/lapis in fixed order; Mesa decorators then add twenty gold veins from Y=32..79. `OreFeature` uses target float sin/cos ellipsoids, floor-based `BlockPos` conversion, and replaces only stone/netherrack.

## Ardosia semantic oracle

Ardosia was inspected at:

- repository: `ardosia/ardosia`
- branch: `main`
- revision: `766f2a2a073889583334758b500b7b6e05acb1f1`

Its fixed-target world model freezes:

- 16×16 horizontal chunks;
- 16×16×16 sections;
- 8 vertical sections;
- world Y range 0..127 (height 128);
- Euclidean chunk mapping for negative block coordinates;
- per-column biome identity;
- separate sky-light and block-light channels;
- immutable/snapshot boundaries above mutable resident terrain.

Cobblestone ports these structural semantics to PHP. It does not port Ardosia's Rust locks, resident-handle graph, game-world leases, or native ownership implementation.

## Matching 0.15.10 source cross-check

The already accepted protocol-84 source oracle was consulted at:

- repository: `KhronosDevs/PocketMine-MP`
- revision: `15272732371b4e7785cc9f45b6274b31198d518e`

Relevant fixed-target facts are:

- the live dimension domain is exactly `0 = Overworld/Normal`, `1 = Nether/Hell`; matching `Level` and `ChangeDimensionPacket` constants expose those IDs, and this 0.15.10 target has no live End dimension;
- StartGame generator ids: `0 = old`, `1 = infinite`, `2 = flat`;
- chunk block coordinates use x/z 0..15 and y 0..127;
- legacy block state is block id 0..255 plus data 0..15;
- extra block data is a separate 16-bit value;
- chunk state carries heightmap, biome id/color, block data, sky light, block light, and generated/populated/light-populated lifecycle flags;
- the historical Flat default preset is `2;7,2x3,2;1;`: bedrock, two dirt layers, grass, biome 1;
- that Flat generator's default spawn is x=128, z=128, y equal to the first air level (4 for the default preset).

The initial Cobblestone package keeps the preset syntax and structural state but intentionally does not import decoration/populator behavior.

The recovered Overworld layer graph was independently cross-checked against `Cubitect/cubiomes@e61f90580cbdd883214a8054670dacae655e59c0` configured for `MC_1_8`. This is validation evidence, not target authority: seven fixed seed/coordinate/area fixtures (including negative coordinates, signed-32-bit seed boundaries, and odd area sizes) match byte-for-byte. The target executable remains the authority for selecting this layer family, seed width, and graph quirks.

Terrain-shape reconstruction was cross-checked against the near-target restored MCPE source `theaperturecat/MCPE-1.0.0-Restored@c095fbd72bbd88d25e6a550c86cbcc659924c4e6` only where its `RandomLevelSource`, MT `Random`, `PerlinNoise`, `ImprovedNoise`, biome height table, and stage ordering matched facts independently recovered from the 0.15.10 executable. The target binary pins the density constants used here: coordinate/height scale `684.412`, min-limit Y scale `×1.25`, min/max normalization `÷256` / `÷512`, 3×3 biome-height smoothing, base size `8.5`, top-four-lattice slide to `-10`, and Overworld sea fill below Y=63. Five independent full-chunk fixtures combine cubiomes' pre-Voronoi biome plane with a standalone target-style MT/Perlin implementation; their air/stone/water bytes match Cobblestone exactly, including negative coordinates, raw signed-32-bit seed boundaries, and a Flower Forest mutation-heavy case.

## Fixed-target block catalog evidence

The shipped `assets-win10.zip` contains `data/resourcepacks/vanilla/blocks.json` with exactly 191 registered block names. The file hashes to `e7b9445531407856c9a3a493f55cdab57814aa0f3eba0abc47eba3cf9a763dc0`; the newline-delimited registry-name sequence hashes to `9da4af43358bd40ebaa7f205a337bb6dae70f7918cec180efd73df5065ee3a49`. The names are retained verbatim, including target-specific spellings such as `pistonArmCollision`, `tripWire`, `glowingobsidian`, `info_update2`, `movingBlock`, and `reserved6`.

The supplied `Minecraft.Win10.DX11.exe` independently contains fixed-target registry vocabulary including `pistonArmCollision`, `nether_brick_fence`, `glowingobsidian`, `info_update2`, and `reserved6`. The binary strings are used as a vocabulary cross-check, not as numeric-ID evidence.

Numeric legacy IDs are pinned to the matching-source `legacy/old-src/block/BlockIds.php`. Asset order is not numeric ID order: for example, `nether_brick_fence` appears near the early asset entries but is legacy ID 113, while `fire` is the final `blocks.json` entry but legacy ID 51. Reconciliation yields exactly 191 unique IDs. Unsupported holes are rejected rather than treated as opaque valid states; notably IDs 36, 84, 119, 122, 130, 137-138, 160, 166, 168-169, 176-177, 188-192, 200-242, and 251-254 are absent.

## Cobblestone API decisions

The client-facing C++ `Level` concept maps to ordinary PHP `Cobblestone\World\World`. `BlockSource` and `ChunkSource` remain explicit interfaces because they form useful semantic ownership/access seams. `MainChunkSource` is the owner-runtime resident index backed by the configured generator.

`GeneratorType` contains the three fixed-target ids because they are part of the StartGame/world vocabulary. `FlatGenerator` is the only complete public terrain generator today. The exact Overworld `BiomeSource`, Infinite `prepareHeights` base terrain, `buildSurfaces` bedrock/top/filler stage, `LargeCaveFeature` carving stage, 3x3 population neighborhood boundary, water/lava lake population, and common ore decoration are implemented underneath it as coarse native mechanisms. Infinite is still not advertised as a public generator until the remaining fixed-target population/features are composed. Old remains unimplemented.

`Dimension` is a separate backed enum with the exact fixed-target IDs `Overworld = 0` and `Nether = 1`. World creation defaults to Overworld, persistent metadata is authoritative on reopen, and StartGame receives the stored world dimension instead of a hard-coded zero. Dimension identity is intentionally implemented before Nether generation/portals so later world semantics have a stable durable domain.

Legacy block state remains represented on hot paths by one scalar `BlockStateId` (`id << 4 | data`). Public semantic identity is the closed backed enum `BlockType`, whose 191 singleton cases carry the exact legacy IDs and exact asset names; `BlockData` is the closed 0..15 metadata nibble; and `BlockState` is immutable `BlockType + BlockData`. Semantic code therefore names blocks instead of passing legacy numeric IDs, while native/chunk/storage/protocol paths keep the compact integer state token. PHP and the native world store reject states whose block ID is not registered for 0.15.10. Block behavior belongs to a later package and is deliberately not modeled here.

The production server now selects a region-sharded native `WorldStore` whenever the extension is loaded. Rust owns the physical chunk planes, biomes, heightmap, sparse extra data, terrain/light revisions, immutable snapshots, and atomic revision-checked patches; PHP remains authoritative for world/gameplay semantics, chunk residency decisions, generator policy, mutation callbacks, and light-propagation rules. The PHP representation is retained as a parity-tested fallback rather than the production storage target.

Immutable `ChunkSectionSnapshot` / `ChunkSnapshot` projections are the bulk read boundary. Large staged mutations switch to a snapshot after a small number of native point reads, while commit remains one patch per changed chunk. Lighting uses one immutable snapshot per touched chunk, scalar staged light levels, exact terrain/light revision validation, and one patch per changed chunk. Protocol-84 initial streaming reads native snapshots directly inside the extension, caches FullChunkData by terrain/light revision, compresses the Batch, and submits it through the session host. The old synthetic probe remains exported only for ABI compatibility and is no longer on the production join path.

## World API parity expansion

`world-api-parity-v1` uses `ardosia/ardosia@766f2a2a073889583334758b500b7b6e05acb1f1` `crates/world` as the semantic oracle for the broader world substrate. Cobblestone now tracks separate terrain/light revisions, chunk-local staged terrain and light edits, immutable light snapshots, `SectionY`, resident chunk cell identity, `LightLayer`/`LightUpdate`/`LightAccess`, and the recovered low-level light propagation order.

Ardosia's recovered block-light table uses dense semantic registry ordinals, while Cobblestone intentionally retains protocol-84 legacy `id:data` states. Those recovered light properties are attached to the same authoritative `BlockType` identities as the exact 191 asset-backed blocks, mapped to legacy IDs through the pinned matching-source `legacy/old-src/block/BlockIds.php`. `BlockLightCatalog` is only the scalar-state adapter used by propagation; semantic metadata lives on `BlockType`. See `WORLD_API_PARITY.md` for the full parity matrix and explicit runtime adaptations.

