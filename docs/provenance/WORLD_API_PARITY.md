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
| Infinite Overworld Village structures | reusable `StructureStartCore` / `StructureStartCache<T>` + `VillageStructureState` + `OverworldVillageStructures` | exact recovered radius-4 discovery, candidate/biome/abandoned gates, full Village topology and block recipes, per-chunk clipping/idempotence, durable start/piece continuation |
| Infinite Overworld Mineshaft structures | reusable structure core/cache + `MineshaftStructureState` + `OverworldMineshaftStructures` | exact radius-8 candidate scan, normal/Mesa topology and vertical placement, Room/Corridor/Crossing/Stairs output, liquid piece removal, rails/webs/spawner, per-chunk continuation |
| Infinite Overworld Stronghold structures | reusable structure core/cache + `StrongholdStructureState` + `OverworldStrongholdStructures` | exact Village-backed first three + additional-grid locator, copied-RNG topology, all Stronghold piece output, portal/spawner state, per-chunk continuation |
| Infinite Overworld scattered structures | reusable structure core/cache + `ScatteredStructureState` + `OverworldScatteredStructures` | exact 32/8 candidate math + raw-biome dispatch; Desert Pyramid, Jungle Pyramid, Swampland Hut; target ground alignment/traps/no-op chest/mob semantics |
| Infinite Overworld ore decoration | native mutable `PopulationNeighborhood` + `OverworldOreDecorator` | exact recovered 3x3 population boundary, biome-decoration reseed, common PE ore sequence/geometry, extra-gravel burst, Mesa extra gold, cross-chunk writes |
| Infinite Overworld biome decoration | `OverworldBiomeDecorator` + native feature/tree owners | exact shared post-freeze reseeded MT stream, cached pre-population heightmap queries, common feature order/counts, target tree families, subclass pre/post hooks, mutated-biome ownership quirks, cross-chunk writes |
| Infinite Overworld composition/exposure | `OverworldInfiniteGenerator` + `Generator\\InfiniteGenerator` + `WorldFactory::infinite()` / `persistentInfinite()` | exact recovered stage order over authoritative 3x3 inputs, shared structure RNG/state, center lifecycle/light completion, cross-chunk install, durable structure-state sidecar, persistent async dependency loading, representative whole-pipeline fixtures |
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

The asset list is not numerically ordered, so each backed `BlockType` case carries its explicit legacy ID while internal metadata preserves the exact asset name. The reconciliation is one-to-one across all 191 public identities. Public PHP/native state input rejects legacy holes. Native generated-world validation has a separately named executable-only allowance for End Portal ID 119, which the target registers and Stronghold PortalRoom can emit; this does not create a public `BlockType` case. The compact state representation remains `(id << 4) | data`; `BlockData` models the exact four-bit metadata domain, and `BlockState` combines the two semantic values without introducing a behavior-class hierarchy.

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

Cobblestone exposes this as `BiomeId` + immutable `BiomeColumn`, stores full `u32` words in both PHP/native chunk backends, preserves the biome-word layout introduced by semantic payload v2 in the current v3 write format, migrates old payload-v1 ID bytes explicitly, and projects the stored words unchanged to protocol 84.

## Fixed-target Overworld biome-source parity

The uploaded 0.15.10 executable establishes the GenLayer-style source family and concrete stage graph: island/fuzzy zoom, repeated land expansion, ocean reduction, snow/climate edges, mushroom/deep-ocean, biome initialization/edges/hills/rare mutation, shore, river branch/mix, smooth, and final Voronoi zoom. The recovered graph preserves the original integer-wrap RNG constants and stage seeds, the 32-bit MCPE world-seed input widened as raw unsigned bits into the 64-bit layer RNG, and the pre-1.13 Hills auxiliary-zoom zero-initialization quirk. Production seed input mirrors `LevelSettings::parseSeedString`: short input uses a random 32-bit default, signed numeric prefixes use their raw 32-bit pattern, exact `-1` is special-cased, and text falls back to the target signed-byte 31× hash. A standalone C++ reproduction matches the PHP `WorldSeed` fixtures including `0`, padded `0`, `-1`, signed extrema, `123abc`, `abc`, `-1foo`, `Cobblestone`, and UTF-8 `é`.

`native/world::OverworldBiomeSource` owns that hot mechanism. PHP exposes a typed `Generator\\BiomeSource` / `OverworldBiomeSource` surface and receives one compact `BiomeArea` byte plane per coarse sample rather than crossing FFI once per column or allocating one object per cell. `BiomeArea::columnAt()` applies the catalog default color only when a semantic generated column is requested. Existing stored `BiomeColumn` words remain authoritative after generation/mutation.

Java 1.8 `cubiomes` is no longer used as a parity oracle: the real 0.15.10 offline world exposed MCPE-specific graph differences. Direct binary recovery shows the medium/lush biome table weights Plains three times and repeated zoom construction reuses `seed + 1`; the user's `mamaMOOSE` offline chunk 0:0 independently locks the resulting final biome plane to FNV-1a `0xdfba91c444c4dfc3` (239 Plains, 17 River) and the target spawn search to X/Z `4:4`. Flat remains preset-driven and intentionally bypasses this Overworld source.

## Fixed-target Infinite terrain-shape parity

The uploaded 0.15.10 executable is authoritative for the `RandomLevelSource::prepareHeights` stage. Cobblestone mirrors its 5×17×5 density lattice, 4×8×4 interpolation order, raw quarter-scale biome sampling, 32-bit-seeded MT19937 stream, exact Perlin bank construction order, biome depth/scale smoothing, and Y<63 still-water fill. Output is an immutable 32,768-state `ChunkTerrainShape` containing only air, stone, and still water.

The MT/Perlin bank construction, density constants, biome height table, and surface-simplex draws retain focused independent target checks. Five complete pre-surface chunks are now explicitly composition-regression fixtures under the corrected MCPE biome graph; the prior full-chunk values were derived from Java `MC_1_8` biome inputs and were retired rather than presented as target-authoritative evidence.

## Fixed-target Infinite surface-building parity

The target executable is authoritative for the following `RandomLevelSource::buildSurfaces` stage. Cobblestone retains the constructor's four-octave surface simplex instead of merely consuming its RNG initialization, then reseeds one MT stream per chunk with the target wrapping constants `341872712` and `132899541`. Column iteration remains X-major/Z-minor because RNG consumption makes ordering observable. `OverworldSurfaceBuilder` consumes the immutable base shape and final Voronoi biome plane and returns one immutable 32,768-state `SurfacedChunk`; no PHP/per-block FFI path is introduced.

The target biome vtables identify the base surface routine and specialized overrides for Extreme Hills, Mesa, Mutated, Mutated Savanna, Swamp, and Taiga. Cobblestone reproduces their fixed-target surface semantics: default grass/dirt and sand→sandstone runs, cold-water ice substitution, 2..5-layer bedrock, stone beach/mycelium/ice-spikes materials, Extreme Hills gravel/stone thresholds, Mega Taiga podzol/coarse dirt, mutated Savanna thresholds, Swamp's fixed-seed biome-info prepass, and Mesa/Bryce red-sand/hardened/stained-clay bands plus pillar noise. Generic mutated biomes delegate to their parent surface as in the target.

Independent validation still checks the four-octave surface-simplex f32 bit patterns against a standalone C++ implementation of the MCPE MT/Simplex draw sequence, while direct material-threshold tests lock the specialized biome rules. Full surfaced chunks are composition regressions under the corrected MCPE graph, with representative default/beach, Desert, Mesa, Bryce, Swamp, Mega Taiga, mutated Savanna, negative-coordinate, and signed-seed coverage. Full-chunk hashes that depended on Java `MC_1_8` biome bytes were retired.

## Fixed-target Infinite cave-carving parity

The 0.15.10 executable exposes `LargeCaveFeature` for Overworld generation and `LargeHellCaveFeature` for Nether generation; no separate Overworld ravine/canyon generator type is present. The recovered `RandomLevelSource` load path applies `LargeCaveFeature` immediately after surface building, making it the complete fixed-target Overworld carving stage for this version.

`OverworldCaveCarver` reproduces the target's world-seed odd X/Z scale derivation, inclusive radius-8 source-chunk scan, wrapping per-source MT reseed, nested cave-count gating, optional room generation, recursive tunnel split, target float trigonometric drift, and `nextGaussianFloat()` draw semantics. Carving preserves the target's unusual raw-buffer behavior: water on the scan boundary is converted to flowing water and aborts that tunnel step; diggable blocks below the geometric Y<10 cutoff become still lava; other diggable blocks become air; thin sand is repaired to sandstone/red sandstone; exposed dirt can become grass; block ID replacement does not clear the metadata nibble.

The standalone C++ `LargeCaveFeature` reconstruction still supplies independent evidence for the target RNG/carving rules, and focused tests cover cross-chunk radius-8 continuity, water-boundary abort, lava cutoff, metadata preservation, and deterministic source scanning. Complete cave-stage chunk hashes are now composition regressions over corrected surfaced inputs; the old Mesa/Ice-Spikes labels and hashes depended on Java-biome surface inputs and are not retained as independent target evidence.

## Fixed-target Infinite lake-population parity

The target `RandomLevelSource::postProcess` first mutates the 3x3 neighborhood with optional lakes before structure/dungeon work. Infinite passes `false` for the restored source's `legacyDevice` constructor flag, so water placement uses the full target Y range and lava attempts are active. Desert/DesertHills skip the water gate entirely; a selected water attempt sets `hasLake` even if `LakeFeature` later aborts, suppressing the lava attempt. Lava uses the target nested Y distribution and unusual Y gate: 60..63 passes directly, while Y>=64 additionally requires 1/10.

`OverworldLakePopulator` preserves the 16x8x16 union-of-ellipsoids mask, air descent and (-8,-4,-8) origin adjustment, boundary validation, still-liquid/air cavity split, exact lava boundary RNG, and cross-chunk writes. The source's skylight-based grass repair is deliberately inert here because fixed-target generated Overworld chunks retain MIN/zero skylight until the later post-process lighting stage.

Independent validation uses a standalone C++ MCPE-MT/LakeFeature oracle over synthetic 3x3 neighborhoods. Hard fixtures match Rust exactly for a negative-coordinate seed-0 water lake (`0x5e20b4e846e48585`, 20 neighboring states changed), Desert suppression at the same center, failed water placement that still suppresses lava, a cross-chunk Y=60 lava lake over dirt (`0xaa580d1c9aa95d65`, 160 lava + 182 boundary stone states), and signed seed `-1` water placement (`0x5afb2180df11d495`, 122 neighboring states changed).

## Fixed-target Infinite Village-structure parity

The target structure stage is historyful. Structure generation creates/caches `StructureStart` instances, and later post-processing asks cached starts to mutate only pieces intersecting the current center chunk. For Village pieces, the actual 0.15.10 `getAverageGroundHeight` clips its sample to that center chunk and uses `BlockSource::getTopSolidBlock(..., false, false)`; the first intersecting chunk therefore fixes the persisted vertical translation. Chunk generation itself is queued with player-distance `_getChunkPriority`, while `_startPostProcessingArea` probes the generated center and its eight neighbors. Cobblestone's serial view preparation preserves the target X-fast GridArea base order and applies the same distance-priority term before generation. `StructureStartCore` preserves source/bounds and target chunk-hash idempotence; generic `StructureStartCache<T>` provides the reusable start container for later structure families, while `VillageStructureState` owns Village-specific durable piece semantics. Target-compatible reload deliberately drops generated-chunk bookkeeping because that vector is not part of the persisted structure tags.

`OverworldVillageStructures` reproduces the fixed-target radius-4 scan, 40/12 spacing math and salt `10387312`, wrapping 32-bit seed behavior, negative-coordinate pre-adjustment, allowed raw-biome gate, 1/50 abandoned-village gate, exact piece weights/counts and expansion RNG, all fixed-target Village piece dimensions/transforms, biome-dependent block substitution, average-ground-height movement, chunk clipping, and the mutable Smithy chest flag. The actual 0.15.10 APK `VillageFeature::isFeatureChunk` uses each raw MT output modulo 28 for spacing; the apparent `raw >> 2` in its Thumb division sequence is only quotient construction, not a pre-modulus transform. The corrected locator is independently anchored by the real `mamaMOOSE` LevelDB, whose Well footprints map to source chunks `(2,3)`, `(1,6)`, and `(0,9)`. Start post-processing also preserves the target GNU libstdc++ 4.9 `std::unordered_map<ChunkPos,...>` iteration semantics instead of Vec discovery order; `ChunkPos` hashes as `(x * 0x1f1f1f1f) ^ z`, making the three overlapping `mamaMOOSE` starts process as `(0,9) -> (1,6) -> (2,3)`. Offline final blocks independently pin that precedence: the later `(2,3)` BookHouse removes the visible `(1,6)` farm overlap, and the `(1,6)` StraightRoad paints GrassPath across the already-placed `(0,9)` TwoRoomHouse top. The later pinned restored source comments out StraightRoad mutation, but the actual 0.15.10 APK `libminecraftpe.so` executes it: the bounded road scan paints biome-adapted GrassPath or WoodPlanks above liquid at the top solid block (ARM Thumb `StraightRoad::postProcess` RVA `0xd6f748`). Cobblestone follows the executable for roads and furnishing rather than the restored comments. The real 0.15.10 x86 binary actively places biome-adapted doors in `SmallTemple`, `BookHouse`, `SmallHut`, both `PigHouse` entrances, and `TwoRoomHouse` (but not `SimpleHouse`/`Smithy`), suppressing those doors in abandoned villages; `LightPost` caps the fence with black wool data 15. `Smithy` alone owns the Village chest path: it places an oriented chest at local `(5,1,5)`, consumes 3..8 target-MT weighted-loot rolls, persists its `Chest` flag, and saves the resulting chest block entity/inventory. The real `TwoRoomHouse` has no chest call or chest field despite the later restored fragment. Protocol-84 chunk streaming appends serialized block-entity NBT after terrain and block-extra data, so generated Smithy chest state is retained in Cobblestone storage and emitted with the chunk. The supplied archive contains villager presentation assets but no Village geometry/template resources, so no template provenance is claimed.

Independent validation does not derive expected topology/block hashes from Rust. A standalone C++ target-MT/candidate/planner oracle fixes representative topology across negative coordinates and signed seed boundaries, including an abandoned seed-0 Village. Separate standalone C++ block oracles over synthetic flat target-format terrain match Rust for an isolated Well across Plains/Desert/Savanna/Taiga style substitutions, an isolated cross-chunk SmallHut, and an isolated abandoned SimpleHouse exercising mossy/web RNG and the target pass-by-value selector quirk. State tests cover repeated apply, post-process idempotence, durable encode/decode, malformed-state rejection, and deterministic replay after target-compatible reload.

## Fixed-target Infinite Mineshaft-structure parity

`OverworldMineshaftStructures` reuses the common historyful structure lifecycle but preserves the Mineshaft-specific radius-8 source scan and candidate stream. Source starts are accepted only after the target clear-draw plus `nextFloat() < 0.004` and distance-gated `nextInt(80)`. `MineshaftPlan` reproduces Room/Corridor/Crossing/Stairs selection, recursive child ordering, collision shortening, depth/distance limits, exact Room Z/Y/X constructor draw order, Mesa room chance and dark-oak style, and the distinct normal versus Mesa vertical translations. `MineshaftPostProcessor` clips each piece to the current center chunk; liquid-edge failures erase pieces from the durable start exactly as `StructureStart::postProcess` does.

Placement preserves Room floor/cavity/entrances/upper-half sphere, Corridor probabilistic roof/webs/supports/torches/rails and generic spider-spawner block, two-floor Crossing cuts/supports, and rotated Stairs carving. The restored target hardcodes structure brightness to zero here, so all `< 8` gates pass and rail probability stays on the 0.7 path. `postProcessMobsAt` exists but its `LevelChunk` invocation is commented out in this target source; minecart-chest entity population is therefore deliberately absent. The shipped archive contains no Mineshaft templates.

Independent validation uses a standalone C++ std::mt19937 candidate/topology oracle for seed `0`, `-1`, and `i32::MIN`, including a forced Mesa/surface topology fixture. A second standalone C++ block oracle matches Rust hashes/counts for Room, rail Corridor, spider Corridor, dark-oak two-floor Crossing, and rotated Stairs on synthetic target-format chunks. Additional tests cover cross-chunk continuation, liquid-driven piece erasure, empty-start reload, mutable spider state persistence, and per-chunk idempotence.

## Fixed-target Infinite Stronghold-structure parity

`OverworldStrongholdStructures` preserves the target's two placement systems. Lazy initialization reseeds the supplied structure RNG to the world seed and finds the first three Strongholds by scanning around the recovered angle/distance sequence for complete valid Village source chunks, including Village's raw-biome gate. Additional Strongholds use the recovered 200-chunk grid, 50..149 inset coordinates, minimum origin distance 10, seed multipliers `784295783249` / `827828252345`, salt `97858791`, and 25% chance. The ordinary radius-8 `LargeFeature` scan still performs the target clear draw before candidacy/start creation.

`StrongholdPlan` reproduces the exact weight table, min-depth/max-count rules, forced first FiveCrossing, 50-depth/112-block reach limits, collision geometry, filler-corridor fallback, random pending-child order, and final `moveToLevel(seaLevel - 5)`. Crucially, `generatePieceFromSmallDoor` copies the MT state before piece selection/constructor draws, preserving the target Android crash-fix compatibility bug where those draws do not advance the parent structure stream. Independent std::mt19937 topology fixtures match Rust exactly for seed `0`, `-1`, `i32::MIN`, `0x12345678`, and an additional-grid Stronghold.

`StrongholdPostProcessor` implements StairsDown, ChestCorridor, FillerCorridor, FiveCrossing, both turns, Library, PortalRoom, PrisonHall, RoomCrossing, Straight, and StraightStairsDown with the target smooth-stone edge selector and full-recipe RNG consumption before chunk clipping. Target `createChest()` is commented out and always false, so no Stronghold chest blocks/loot are invented; ChestCorridor still persists its placement flag. PortalRoom rolls twelve independent eye bits, places a 3×3 End Portal only when all eyes are present, and persists its generic mob-spawner block flag while the silverfish entity assignment remains commented out. Independent C++ block-array oracles match Rust hashes for all simple pieces, RoomCrossing variants 0/1/2, short+tall Library, and PortalRoom. State tests cover cross-chunk continuation, per-chunk idempotence, malformed-state rejection, mutable portal-spawner reload, and reset of transient generated-chunk bookkeeping.

The shipped archive has no Stronghold structure templates. The executable does register `end_portal` as legacy ID 119 even though shipped `blocks.json` omits it; Cobblestone therefore keeps 119 outside the 191-case public `BlockType` catalog while allowing it in explicitly internal generated-world validation.

## Fixed-target Infinite scattered-structure parity

`OverworldScatteredStructures` preserves the radius-8 `LargeFeature` scan and target 32/8 spacing math, including negative-coordinate pre-adjustment and the single clear draw performed before feature candidacy. Raw chunk-center biome IDs select exactly three 0.15.10 families: Desert Pyramid for Desert/Desert Hills, Jungle Pyramid for Jungle/Jungle Hills, and Swampland Hut for Swampland/Swampland Mutated. The actual 0.15.10 executable contains RTTI for those three pieces and no `Igloo`; the restored 1.0 source's Igloo support is later content, consistent with historical Bedrock asset reports that introduce the four Igloo NBT templates at 0.17.0.1 / the 1.0 beta.

All three target pieces use the source's fixed South orientation. Desert Pyramid retains its Y=64 foundation/towers/tomb/TNT geometry and the target TODO that collapses both decorative clay colors to metadata zero. Jungle Pyramid persists first-chunk average-ground alignment, full moss/cobble selector RNG, tripwire/redstone/piston puzzle output, and the two active dispenser blocks; dispenser loot population is commented out but the durable trap flags become true when their chunks are processed. Target `createChest()` is a no-op, so Desert and Jungle chest flags remain false. Swampland Hut persists first-chunk alignment plus one Y offset, spruce/log geometry and downward support columns; cauldron contents and witch spawning remain commented/dormant.

Spacing/RNG remains independently covered by the standalone target std::mt19937 oracle. Biome-qualified locator coordinates are now regression snapshots under the binary/offline-world MCPE graph rather than a `cubiomes` oracle. A separate standalone target-format block oracle still matches the center-chunk hashes `0x8e10fa9a2eb3f1d5` (Desert), `0x1789bd78f317c071` (Jungle), and `0x2f7e83e9379a9ced` (Hut), plus sentinel block counts and Jungle trap flags. State tests cover durable codec semantics and first-chunk height persistence.

## Fixed-target exclusion: Ocean Monument

Ocean Monument is intentionally not implemented for the 0.15.10 target. Direct executable inspection finds no Ocean Monument or Guardian runtime vocabulary; shipped vanilla `blocks.json` also lacks the Prismarine and Sea Lantern registrations required by that generator. Future-content textures in themed resource packs are treated as dormant assets, not registry or world-generation evidence. Mojang's official MCPE/Win10 0.16 release notes list Ocean Monuments, Guardians/Elder Guardians, Prismarine variants, and Sea Lantern as new 0.16 features. Consequently the complete `OceanMonumentFeature` subsystem present in the pinned restored 1.0 source is explicitly post-target and excluded from parity.

## Fixed-target Infinite Monster Room parity

`OverworldMonsterRoomPopulator` implements the immediate dungeon stage after Village → Mineshaft → Stronghold → Scattered post-processing. The target makes eight attempts on the continuing population MT stream, choosing X/Z at center-chunk origin + 8..23 and Y across 0..127 before drawing 2..3 block room radii. That +8..23 range is the global 16-block `CHUNK_WIDTH`, not the separate four-block terrain interpolation lattice, so a room may originate in the east/south neighboring chunk while remaining inside the existing 3x3 population boundary.

`MonsterRoomFeature` requires solid material across its complete floor and validated ceiling and exactly 1..5 two-block side openings. It then preserves the target's asymmetric placement loop (the validated Y+4 ceiling is not rewritten), cobblestone shell, 3/4 mossy floor selection, air cavity, up to two chests with exact facing metadata, and generic mob-spawner block. Chest loot-table filling and spawner entity-id assignment are commented out in the recovered branch; Cobblestone therefore creates no dungeon loot/spawner target semantics and deliberately consumes no dormant entity-selection RNG. Direct executable inspection independently exposes `MonsterRoomFeature` and the MobSpawner runtime family; the supplied Win10 assets contain chest/spawner presentation resources but no `simple_dungeon` loot table or Monster Room template.

Independent validation uses a standalone C++ std::mt19937/block-array oracle rather than Rust-derived expected output. A direct room crossing center/east/south chunk boundaries matches full-neighborhood hash `0x08f7a006e78172d2`, including one north-facing chest. Full eight-attempt population fixtures match at signed seed `-1`, center `-1:-1` (`0x77acd8635b99db16`, 245 neighboring states changed), and `i32::MIN`, center `7:-9` (`0xa7ba4f35cb1996bf`, 270 neighboring states changed). These fixtures exercise negative coordinates, signed seed boundaries, candidate-origin spill into neighbors, exact successful-room RNG consumption, chest placement, and the commented-out spawner draw.

## Fixed-target Infinite freeze/frost parity

`OverworldFreezeFrostPopulator` reproduces the 0.15.10 `freezeFrostProcess` immediately after Monster Rooms and before the independent biome-decoration reseed. This behavior is pinned from the actual Win10 0.15.10 executable rather than copied from the later restored 1.0 branch: the latter has `BlockSource::shouldFreeze` commented out, while the target binary executes the temperature, light, water-ID, and metadata tests before writing ice. The stage consumes no RNG and writes no top-snow blocks.

The target scans center-local X then Z over all 16×16 columns. Its rain-height lookup starts at Y=127, ignores Y=0, regards `Material::getBlocksMotion() || Material::isLiquid()` as blocking rain, and clamps a found Y+1 to 127. Freeze then examines the block below that height, accepts only the fixed PE biome temperatures <=0.15f, requires block light <10, and replaces only data-0 flowing/still water with legacy ice state 79:0. Generated chunks still have zero block-light nibbles here because emitter propagation and final light/height processing run later, making the binary's light predicate true throughout this generation boundary. Pocket Edition has no altitude cooling in `Biome::getTemperature()` here; the exact cold set is IDs 10, 11, 12, 13, 26, 30, 31, 140, and 158.

Independent validation uses a standalone C++ target-format block-array oracle. A negative-coordinate Ice Plains fixture with decorations, flowing/still water metadata variants, Y=127 cap behavior, and an unreachable Y=0 water cell matches full-neighborhood hash `0xa142b37de83230d5`, producing 128 ice blocks while preserving 66 still-water states and the Y=0 sentinel. A warm-biome fixture at center `7:-9` matches `0x88e7cec4c9a02325` and preserves all 256 water cells. Separate domain tests lock the exact cold-biome set and the generated-material rain-blocking predicate.

## Fixed-target Infinite ore-decoration parity

The target population boundary is a 3x3 neighborhood rather than a center-chunk-only write surface. `PopulationNeighborhood` therefore retains nine carved chunk planes as one transient generation-owned unit. `OverworldOreDecorator` accepts that supplied unit instead of manufacturing its own prerequisite state, allowing later lake/structure/dungeon issues to run before biome decoration in the exact target order. The ore stage reseeds MT from the world seed using the target odd X/Z scales and center chunk coordinates immediately before decoration.

The recovered common sequence is dirt 10×33, gravel 8×33, optional PE extra gravel 80×33 below Y50, diorite/granite/andesite 10×33 below Y80, coal 20×17, iron 20×9 below Y64, gold 2×9 below Y32, redstone 8×8 below Y16, diamond 1×8 below Y16, and lapis 1×7 using the target triangular Y distribution around 16. Mesa-family decorators append 20 gold 9-block veins over Y32..79. Every `OreFeature` draw and float/floor ellipsoid operation is preserved, and writes may cross from the center into any required neighbor in the 3x3 boundary.

Independent validation uses a standalone C++ MCPE-MT/OreFeature oracle over synthetic all-stone 3x3 target-format neighborhoods so failures isolate this stage from terrain/cave noise. Five hard fixtures match exactly across negative coordinates and signed seed boundaries. The seed-0 `-1:-16` fixture exercises the PE extra-gravel branch (7333 gravel states, 8072 changed neighbor states); the seed-0 Mesa fixture at `-58:-128` exercises the extra-gold override (137 gold states). All fixtures produce neighboring-chunk writes, and a separate integration test confirms real carved neighborhoods retain those cross-chunk mutations.

## Fixed-target Infinite biome-decoration parity

`OverworldBiomeDecorator` owns the full `Biome::decorate` call after freeze/frost and uses one independently reseeded population MT stream for ores plus every subsequent decorator feature. `PopulationNeighborhood` caches the target generation heightmap before population starts; later feature writes intentionally do not update that cache, matching the height queries observed by decorator helpers. Common decoration preserves the target order and counts for sand/clay/gravel disks, tree families, PE grass scatter, huge/small mushrooms, flowers, grass, dead bushes, water lilies, reeds, pumpkin, cactus, and water/lava springs.

Biome-specific hooks preserve the fixed-target ordering around that common pass: Plains/Sunflower, Forest/Roofed/Flower Forest, Taiga/Mega Spruce, Savanna, and Ice Spikes run their recovered pre-decoration behavior; Desert, Extreme Hills, and Jungle append their well, emerald/monster-egg, and melon/vine behavior afterward. Generic mutated biomes retain the target ownership quirk where `MutatedBiome::decorate` invokes the contained decorator directly instead of the contained subclass override. Direct 0.15.10 executable RTTI confirms the implemented feature/tree families and lacks `FossilFeature`, so later fossil generation is excluded.

Independent standalone C++ fixtures pin the combined RNG/placement behavior rather than validating Rust against itself. A synthetic Ocean/common stream fixture at seed 36 matches full-neighborhood hash `0x20ef402f27d495ff` and seven yellow-flower writes; a Desert seed-2471 fixture matches `0xe4bdf751cb876a87`, including one dead bush, four cactus blocks, and a complete 70-sandstone/12-slab/5-water well; and a direct viney Oak seed-1 fixture matches `0xf216b0707794ecda` with five logs, 54 leaves, and eight vines.

## Fixed-target Infinite post-decoration finalizer parity

`OverworldPostDecorationFinalizer` owns the target boundary after `Biome::decorate`. It preserves the executable order `_fixWaterAlongEdges` → generation-time Seasons/top-snow → generation tick drain → final light/height processing. The edge-water pass scans the sixty perimeter columns against the pre-population cached heightmap, converts the first still-water block in each vertical run to flowing water, and schedules its target delay-1 liquid tick. `SpringFeature` likewise performs its target immediate dynamic-liquid tick and shares this generation-only queue; this does not implement the general runtime scheduled/random tick system.

Generation Seasons uses the fixed `89328` snow-noise seed and five-octave 3D simplex path, the target snow-biome accumulation/min/max profiles, target rain-height/placement rules, and covered top-snow extra-data semantics. The final lighting owner then recomputes the center heightmap from fixed-target light-block properties and produces packed sky/block-light nibble planes over the shared 3x3 neighborhood so cross-chunk emitters can affect the center before persistence.

Independent standalone C++ fixtures pin each block-visible family. The edge-water two-run fixture matches center-state hash `0x7acae6ad6447a3b5`; the covered ten-layer top-snow fixture matches state+extra-data hash `0xd764a759a386f65e`; and the final light/height fixture with water attenuation plus an east-neighbor torch matches packed height/sky/block-light hash `0xeb4cef1c923ece33`.

## Deliberate adaptations

Rust's `TerrainReadGuard`, `TerrainWriteGuard`, `LightReadGuard`, and `LightWriteGuard` exist because Ardosia's resident terrain/light cells are shared behind Rust locks. Cobblestone's architecture requires one authoritative PHP runtime owner for mutable gameplay state, so exposing lock guards to ordinary PHP would violate the project API invariant. Direct owner-local terrain/light facades replace those guard types.

Cobblestone does not expose mutable section objects through a `section_mut` equivalent because direct section mutation could bypass height-map and revision invariants. Equivalent block/terrain edit operations are available through `Chunk`, `ChunkTerrain`, and `TerrainEdit`.

Cobblestone retains height map, sparse extra block data, generated/populated/light-populated flags, and protocol-friendly snapshot planes. These are fixed-target requirements beyond the minimal Ardosia `crates/world` terrain model and do not weaken parity.

## Validation boundary

Parity is not considered verified until the relevant Cobblestone revision passes canonical local `composer verify`, including native-world, world parity, mutation, and light smokes.
