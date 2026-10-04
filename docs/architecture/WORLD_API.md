# World, chunks, blocks, generation, lighting, and storage API design

## Status

This document defines the intended public and internal design direction for:

- src/World;
- src/World/Generator;
- src/World/Light;
- src/World/Mutation;
- the native WorldStore in native/world;
- native/storage;
- the world/storage portions of native/extension.

Existing mechanism details remain documented in WORLD_SYNC.md and WORLD_STORAGE.md.

This document focuses on the semantic API these mechanisms should support.

## World as the semantic root

World represents one gameplay world.

It should own or expose:

- identity and metadata;
- time and spawn state;
- block/chunk access;
- entity queries;
- generation policy;
- mutation coordination;
- persistence lifecycle.

Ordinary code should not need a WorldManager to perform world operations.

A World is durable semantic state, so a dedicated object is appropriate.

## Opening and creating worlds

Opening durable state and creating new state are different operations.

The API should make that distinction visible.

A target shape is:

~~~php
$world = World::open('worlds/world');
~~~

and:

~~~php
$world = World::create(
    path: 'worlds/new-world',
    name: 'New World',
    seed: $seed,
    generator: $generator,
);
~~~

The exact static/factory shape may evolve while persistence composition is still changing.

The important rules are:

- open never silently recreates incompatible/missing durable state;
- create never overwrites an existing world without an explicit policy;
- stored metadata wins over creation defaults on reopen;
- storage I/O does not block the owner runtime in steady-state gameplay paths.

WorldFactory may remain an internal composition helper if useful, but ordinary plugin code should not need to understand concrete world collaborators.

## World identity

A world has a stable WorldId independent of display name and filesystem path.

Display names may change.

Paths are persistence locations, not gameplay identity.

Native handles may back identity internally, but stale-handle mechanics remain below the public semantic layer.

## Coordinates

Use explicit immutable values for coordinate domains:

~~~text
BlockPos
ChunkPos
RegionId
Position
Rotation
Bounds / Aabb
~~~

Do not use a generic array such as [x, y, z] as a public contract.

BlockPos is integral block space.

Position is continuous entity/world space.

ChunkPos is chunk space.

These distinctions prevent accidental unit mixing.

## Block access

The common API should be direct:

~~~php
$state = $world->block($position);
$world->setBlock($position, $state);
~~~

A scalar BlockStateId may remain the hot internal currency.

The ergonomic public `BlockState` is typed rather than numeric:

~~~php
$stone = BlockType::Stone->state();
$planks = BlockType::Planks->state(BlockData::Two);

$state->type;
$state->data;
~~~

`BlockType` is a backed enum whose numeric value is the exact fixed-target legacy block ID. `BlockData` is the closed 0..15 metadata domain. `BlockStateId::encode()` / `decode()` are the explicit bridge to the compact scalar representation and should stay below ordinary semantic gameplay code.

## Block types and state

Do not create one PHP subclass per block state.

Prefer:

- BlockType as catalog identity/behavior metadata;
- BlockState as immutable placed state;
- BlockStateId as compact hot-path/native representation.

A block's mutable gameplay state belongs to the world/chunk, not to a long-lived mutable Block object.

A temporary Block view may exist for convenience if it clearly represents world + position + state.

## Block behavior

Behavior such as placement, breaking, activation, drops, and updates belongs to gameplay semantics.

The world storage layer stores state.

Gameplay code interprets interactions.

A future block behavior registry may map fixed-target BlockType values to semantic handlers without turning each stored block into an object.

Protocol IDs remain catalog/codec concerns.

## Coarse edits

Related writes should support one scoped operation:

~~~php
$world->edit(function (WorldEdit $edit): void {
    $edit->setBlock($a, $stone);
    $edit->setBlock($b, $air);
    $edit->setBiome($chunk, $biome);
});
~~~

WorldEdit represents one semantic mutation boundary.

The implementation may:

- validate first;
- acquire affected chunk ownership;
- apply one native patch/batch;
- update heights/light/revisions;
- append one bounded change journal entry/coalesced set;
- publish viewer updates after commit.

This is the preferred path for multi-block gameplay behavior.

## Single mutations

Simple one-block operations stay simple:

~~~php
$world->setBlock($position, $state);
~~~

The implementation may internally compile that into the same mutation machinery.

Users should not need a transaction object just to place one block.

## Mutation results

Mutations should return only information callers actually need.

World edits return the callback value directly; do not reintroduce a generic mutation-result data bag.

Useful semantic outcomes may include:

- previous state;
- whether anything changed;
- revision/snapshot identity for advanced code.

Internal terrain/light/persistence revision bookkeeping does not automatically belong in the public result.

## Chunk access

Chunk is a semantic view of one 16×16×128 fixed-target chunk.

Normal code should be able to get a resident chunk without touching residency cells:

~~~php
$chunk = $world->chunk($position);
~~~

If the chunk is not yet available because durable state is loading, the behavior must be explicit.

Current ChunkLoadPending semantics are acceptable for synchronous access.

Higher-level helpers may suspend/retry cooperatively where the caller has a Task context.

## Scoped residency

For temporary operations that require a chunk to remain resident:

~~~php
$world->withChunk(
    $position,
    function (Chunk $chunk): void {
        // pinned for this scope
    },
);
~~~

For long-lived ownership:

~~~php
$lease = $world->pinChunk($position);

try {
    $chunk = $lease->chunk();
    // ...
} finally {
    $lease->release();
}
~~~

ChunkLease is a lifetime handle.

Internal ResidentChunkCell and pin counters do not need to be public concepts.

## Chunk snapshots

Immutable ChunkSnapshot is the sharing/read boundary.

Snapshots may cross native worker/network boundaries because they do not transfer mutable authority.

Snapshot cost must be understood.

The current coarse Arc<ChunkData> copy-on-write representation can deep-clone roughly an entire chunk payload on first mutation while a snapshot is alive.

The likely future direction is more granular section/plane sharing if benchmarks justify it.

The public snapshot API should not depend on which internal copy-on-write granularity wins.

## Chunk sections

A 16×16×128 chunk naturally permits 16-high section granularity internally.

If measured copy-on-write pressure warrants it, terrain/light planes may be stored as independently shared sections.

That can reduce first-write clone cost from whole-chunk scale to touched-section scale.

This is an internal representation optimization, not a new plugin-facing chunk hierarchy requirement.

## Height map

Height maintenance should be incremental where possible.

For one block change:

- if placing above the current height, raise directly;
- if changing below the current height, no height rescan is needed;
- if removing/changing the current top block to non-solid, scan downward only until the next valid top.

Bulk fills/patches may recompute touched columns.

The API remains WorldEdit/setBlock regardless of mechanism.

## Lighting

Lighting is a world mechanism with gameplay-visible results.

Plugins should not normally call a LightEngine manager.

Block mutations mark/produce required lighting work.

World snapshots expose resulting light values where needed.

Advanced code may query:

~~~php
$world->skyLight($position);
$world->blockLight($position);
~~~

if those scalar reads prove useful.

Lighting propagation should remain isolated from arbitrary plugin execution and may use native/measured paths.

## Generation

Generator is a small semantic interface.

A target contract is:

~~~php
interface Generator
{
    public function generate(ChunkDraft $chunk): void;
}
~~~

ChunkDraft is mutable generation-owned state that has not yet entered normal resident world ownership.

Generation should not mutate an already-live Chunk through thousands of public scalar calls.

Generation produces a coarse chunk result that is imported/committed once.

## Generator inputs

Generator configuration should be immutable typed data.

Examples include seed and flat preset values.

Do not pass Server or global service locators into generators merely for convenience.

Randomness must be deterministic from world seed + coordinates where fixed-target generation requires it.

### Biome sources

Biome selection is separate from terrain material generation. `Generator\\BiomeSource` answers biome identity, while a terrain generator decides heights/materials/features. The exact fixed-target Overworld implementation is `OverworldBiomeSource`; it samples natively in coarse areas/chunks and returns an immutable `BiomeArea` that keeps the compact byte plane until typed `BiomeId` / `BiomeColumn` access is requested.

~~~php
$source = new OverworldBiomeSource($seed);
$area = $source->chunk($chunkPos); // one native 16x16 sample
$biome = $area->idAt(3, 7);
$column = $area->columnAt(3, 7); // catalog default fixed-target color
~~~

Generated columns use the catalog default color only when the column is first created. Once a chunk is resident/persisted, its stored biome ID + RGB word is authoritative and must not be recomputed by the source. Flat generation remains driven by its preset biome and does not use `OverworldBiomeSource`.

### Infinite Overworld terrain stages

The fixed-target Infinite generator is implemented in stages rather than exposing a partially correct public generator. Native `OverworldTerrainShape` owns the recovered `RandomLevelSource::prepareHeights` hot mechanism: it samples the biome source's raw pre-Voronoi 1:4 plane, evaluates the exact fixed-target MT/Perlin density lattice, and returns one immutable 16×128×16 `ChunkTerrainShape` of compact state IDs containing only air, stone, and still water at sea level 63.

Native `OverworldSurfaceBuilder` owns the following recovered `RandomLevelSource::buildSurfaces` stage. It consumes the base shape, final 1:1 biome IDs, the world-seeded four-octave surface simplex, and the target chunk-seeded MT stream. The result applies bottom bedrock plus exact biome surface semantics including sand/sandstone, stone beaches, mycelium, ice-spikes snow, Extreme Hills gravel/stone thresholds, Mega Taiga podzol/coarse dirt, mutated Savanna thresholds, Swamp's special surface prepass, and Mesa/Bryce red-sand/clay bands/pillars.

Native `OverworldCaveCarver` owns the next recovered `LargeCaveFeature` stage. It scans source chunks in an eight-chunk radius around the target chunk, reseeds the fixed-target MT stream per source chunk, and applies deterministic rooms/tunnels into the surfaced state plane. Water on the carve boundary aborts that tunnel step while being converted to flowing water, carved cells below the target cutoff become still lava, and the target's thin-sand plus exposed-grass repair rules are preserved. Block ID replacement intentionally keeps the legacy metadata nibble because the target stores IDs/data separately.

Population is not a single-chunk mutation boundary. The fixed target post-processes a 3x3 chunk neighborhood, so native `PopulationNeighborhood` keeps all nine generation-owned chunk planes together while features run. `OverworldLakePopulator` owns the first recovered post-process mutation stage: it applies the target population reseed, non-desert 1/4 water-lake attempt, failed-water `hasLake` suppression rule, conditional lava-lake attempt, and exact `LakeFeature` cavity/boundary writes. `LakeFeature` contains a skylight-gated dirt-to-grass repair, but generated Overworld chunks still carry MIN/zero skylight at this point and the final light pass runs later, so that branch cannot change fixed-target lake output here.

Village is the first recovered historyful structure stage after lakes. Native `StructureStartCore` owns reusable source/bounds/generated-chunk semantics, `StructureStartCache<T>` owns the reusable cached-start container, and `VillageStructureState` persists Village-specific starts/pieces separately from `PopulationNeighborhood`. `OverworldVillageStructures::apply()` performs target source discovery/start creation; `post_process()` applies only cached pieces intersecting the center chunk, using the supplied 3x3 neighborhood as the block-access boundary while clipping target writes to that center chunk exactly as `StructurePiece::postProcess` does. Generated-chunk idempotence is transient across reload by target design; durable piece bounds/height/style/constructor state survive.

`OverworldMineshaftStructures` is the second historyful structure stage and reuses the same core/cache contract with family-specific `MineshaftStructureState`. Its apply path scans the target radius-8 source square and caches normal/Mesa Mineshaft plans; its post-process path mutates only intersecting center-chunk slices and persists mutable piece state such as spider-spawner placement. A crate-internal shared-random entrypoint is retained so the final Infinite composer can run Village → Mineshaft → later structure families on the single chunk-seeded structure RNG stream required by the target, while isolated family tests can still reseed deterministically.

`OverworldStrongholdStructures` is the third historyful structure stage. It preserves the PE-specific first-three placement under valid Village source chunks, the later 200×200-grid/25% expansion system, the target by-value piece-selection RNG quirk, all Stronghold piece families, and durable mutable chest/spawner flags. Its crate-internal shared-random entrypoint continues the Village → Mineshaft stream for final Infinite composition. The executable also registers internal End Portal block ID 119 even though shipped `blocks.json` omits it, so native generated world state accepts that hidden block while public `BlockType`/PHP state input remains limited to the 191 asset-backed identities.

`OverworldScatteredStructures` is the fourth target structure stage. It reuses the same cache/core model with `ScatteredStructureState`, performs the radius-8 32/8 candidate scan, dispatches the fixed-target Desert Pyramid/Jungle Pyramid/Swampland Hut family from raw center biomes, and persists the historyful first-chunk ground height plus Jungle dispenser flags. Igloo is intentionally absent because the 0.15.10 executable lacks that piece and the template-backed Igloo family appears only in later 0.17/1.0 assets. The family exposes the same crate-internal shared-random post-process entrypoint for final structure ordering.

`OverworldOreDecorator` later mutates the same supplied neighborhood after independently reseeding immediately before biome decoration, applying the common ore order/counts/depth distributions, PE extra-gravel branch, and Mesa extra-gold override. Keeping the neighborhood independent from either feature owner preserves the real order for upcoming structures/dungeons between lakes and biome decoration.

These stages remain coarse native mechanisms; PHP does not perform per-block FFI calls. Cobblestone still does not advertise `InfiniteGenerator` / `WorldFactory::infinite()` because Ocean Monument structures, dungeons, freeze/frost, and remaining biome decoration/features are client-visible target semantics that are not yet composed.

## Generation scheduling

Missing durable chunks may require generation.

The high-level state machine is:

~~~text
unknown
  ↓
load requested
  ↓
durable chunk found -> import
  or
confirmed missing -> generate
  ↓
resident clean/dirty state
~~~

Do not generate immediately while durable state is still unresolved.

This avoids overwriting existing worlds after asynchronous load delays.

## Persistence transparency

World persistence is native mechanism, not a plugin-facing region-file API.

Normal gameplay mutates World.

Dirty state is tracked automatically.

Storage workers save immutable/coarse chunk records.

Plugins should not need to manually serialize chunks after edits.

## Dirty tracking

Persistence should not discover dirty work by scanning every resident chunk each tick once world sizes become material.

The target mechanism is an explicit bounded dirty-candidate queue/set:

- mutation marks a chunk dirty;
- transition into unsaved state queues the chunk once;
- storage tick consumes up to budget;
- successful save updates watermark and removes/requeues only if newer changes appeared.

This mirrors the existing compaction queue/set approach.

## Save semantics

Saving a chunk publishes a durable version.

A save completion for revision N must not accidentally mark revision N+1 clean.

Dirty watermarks/revisions remain the authority.

Save workers consume immutable snapshots/encoded records rather than mutable PHP objects.

## Flush

World shutdown may request:

~~~php
$world->flush();
~~~

or server shutdown may own this automatically.

Flush means durable state required by the persistence contract has reached a safe terminal condition or a typed failure is returned.

It must not mean "scan forever until nothing changes" while gameplay is still mutating the world.

Shutdown first stops new gameplay mutation, then flushes.

## Compaction

Region compaction remains a native storage maintenance concern.

Its queueing, thresholds, exclusion of active regions, and atomic publication stay below gameplay API.

Administrative diagnostics may expose compaction status without exposing file offsets or index internals.

## World metadata

Metadata includes fixed world identity/configuration needed to reopen faithfully.

It should be typed and versioned.

Creation defaults do not override stored metadata.

Metadata publication must follow the durability rules defined in WORLD_STORAGE.md.

## World change publication

World mutation and persistence are separate concerns.

A mutation creates gameplay-visible change immediately after semantic commit.

Persistence may happen later.

The world change journal/projection feeds viewer synchronization independently from save state.

This separation should remain explicit.

## Viewer updates

PHP owns which players view which chunks.

Native code may convert a committed world change into efficient protocol-84 forms:

- UpdateBlock for small terrain-only changes;
- complete/revision-cached chunk data when broader state changed.

Plugins should not marshal viewer packet deltas block by block.

## Biomes and extra data

MCPE 0.15.10 stores one 32-bit biome word per X/Z column, not only a biome ID. The high byte is the registered biome ID and the low 24 bits are an independently mutable RGB color. Cobblestone therefore models:

- `BiomeId` as one of the 60 biomes registered by the exact 0.15.10 target;
- `BiomeColumn` as immutable placed column state containing `BiomeId + RGB`; and
- the chunk biome plane as 256 full biome words.

`Chunk::biome()` / `World::biomeAt()` remain ergonomic identity reads. `setBiome()` changes only the high-byte identity and preserves the existing RGB, matching the historical chunk semantics. `biomeColor()` / `setBiomeColor()` expose the independent color channel, while `biomeColumn()` / `setBiomeColumn()` replace both together.

Flat generation initializes each column from the catalog default color, but later ID/color mutation is not re-derived from a global lookup. Immutable snapshots retain the full words and expose the old 256-byte ID projection only as a compatibility view.

Protocol 84 FullChunkData emits the stored words big-endian unchanged. The world layer must never regenerate biome color during packet encoding.

Block-extra-data remains separate sparse per-block semantic state. Raw numeric maps may remain available only in advanced/compatibility surfaces.

## World time and environment

Common world properties belong directly on World:

~~~php
$world->time();
$world->setTime($time);
$world->spawn();
$world->setSpawn($position);
$world->dimension(); // Dimension::Overworld or Dimension::Nether
~~~

`Dimension` is the closed fixed-target identity domain (`Overworld = 0`, `Nether = 1`). It is durable world metadata and is projected into protocol-84 StartGame; it does not imply that Nether generation, portals, or cross-dimension entity transfer are implemented yet.

Difficulty, weather, game rules, and other environment state should be added only when fixed-target behavior is implemented.

Do not create empty manager abstractions for unimplemented mechanics.

## Entity integration

World owns spatial residence of entities, but entity semantics are specified in GAMEPLAY_API.md.

Expected operations include bounded queries:

~~~php
$world->entities(
    $bounds,
    function (Entity $entity): void {
        // ...
    },
);
~~~

World storage and entity persistence may use separate records internally.

Chunk residency must not accidentally define Player/Entity object lifetime.

## Block/entity interaction boundary

Block storage answers "what state is at this position?"

Gameplay answers "what happens when a player breaks/uses/steps on it?"

This keeps the native world store independent from PHP plugin behavior.

## Revisions

Terrain/light/chunk revisions are useful internal consistency and cache keys.

They should stay available to advanced snapshot/network code.

Ordinary plugin code should not need to compare revision counters to mutate a world.

Optimistic revision checks may be exposed only where concurrent/off-owner workflows genuinely require them.

## Ownership

Authoritative mutable world/chunk state has one owner runtime.

Wrong-owner mutation is routed or rejected.

Immutable snapshots may cross workers/runtimes.

A native handle gives identity; it does not grant mutation rights from arbitrary PHP runtimes.

## FFI shape

Hot world work should prefer coarse operations:

~~~text
apply patch
snapshot chunk
prepare/load batch
take save completions
flush change projection
encode initial view
~~~

Avoid one FFI call per block in generation, lighting propagation, bulk edits, persistence scans, or chunk streaming.

Scalar access remains useful for ordinary occasional gameplay logic.

## Testing

World tests should cover:

- coordinate boundaries;
- block/state parity;
- bulk edit atomicity;
- revision behavior;
- dirty watermark races;
- async load vs generation;
- save completion after newer mutation;
- chunk pin/eviction behavior;
- snapshot immutability;
- generation determinism;
- height-map updates;
- light propagation parity;
- storage reopen/compaction;
- protocol-84 chunk projection.

Performance benchmarks should separately measure scalar ergonomics and coarse hot-path operations.

## Design summary

The world layer should converge on:

> World is the semantic root. Chunk and BlockState are gameplay views. WorldEdit is the coarse mutation boundary. Snapshots cross concurrency boundaries. Generation produces whole chunk drafts. Lighting and storage are mechanisms below the semantic API. Residency, revisions, queues, and region files stay internal unless advanced code truly needs them.
