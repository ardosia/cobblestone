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

- StartGame generator ids: `0 = old`, `1 = infinite`, `2 = flat`;
- chunk block coordinates use x/z 0..15 and y 0..127;
- legacy block state is block id 0..255 plus data 0..15;
- extra block data is a separate 16-bit value;
- chunk state carries heightmap, biome id/color, block data, sky light, block light, and generated/populated/light-populated lifecycle flags;
- the historical Flat default preset is `2;7,2x3,2;1;`: bedrock, two dirt layers, grass, biome 1;
- that Flat generator's default spawn is x=128, z=128, y equal to the first air level (4 for the default preset).

The initial Cobblestone package keeps the preset syntax and structural state but intentionally does not import decoration/populator behavior.

## Cobblestone API decisions

The client-facing C++ `Level` concept maps to ordinary PHP `Cobblestone\World\World`. `BlockSource` and `ChunkSource` remain explicit interfaces because they form useful ownership/access seams. `MainChunkSource` is currently an in-memory resident source backed by the configured generator.

`GeneratorType` contains the three fixed-target ids because they are part of the StartGame/world vocabulary. Only `FlatGenerator` is implemented. Old/Infinite are not advertised as implemented generators.

`BlockState` is a fixed-target state token (legacy id + data), not block behavior. Block behavior belongs to a later `block` package.

The follow-up `world-protocol84-stream-v1` slice adds immutable `ChunkSectionSnapshot` / `ChunkSnapshot` bulk projections. PHP remains authoritative for chunk selection, block/data state, biome identity, heightmap, sky/block light, sparse extra data, and revision. PHP's section-concatenated Y/Z/X planes already match protocol-84 `ORDER_LAYERED`; Rust validates those planes, builds the remaining FullChunkData wire fields, compresses the Batch, and submits it through the session host. The old synthetic probe remains exported only for ABI compatibility and is no longer on the production join path.

## World API parity expansion

`world-api-parity-v1` uses `ardosia/ardosia@766f2a2a073889583334758b500b7b6e05acb1f1` `crates/world` as the semantic oracle for the broader world substrate. Cobblestone now tracks separate terrain/light revisions, chunk-local staged terrain and light edits, immutable light snapshots, `SectionY`, resident chunk cell identity, `LightLayer`/`LightUpdate`/`LightAccess`, and the recovered low-level light propagation order.

Ardosia's recovered block-light table uses dense semantic registry ordinals, while Cobblestone intentionally retains protocol-84 legacy `id:data` states. The 191 recovered identities are therefore mapped to legacy IDs through the pinned matching-source `legacy/old-src/block/BlockIds.php`; unsupported legacy IDs are rejected instead of guessed. See `WORLD_API_PARITY.md` for the full parity matrix and explicit runtime adaptations.

