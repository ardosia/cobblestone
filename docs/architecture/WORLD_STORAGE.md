# Cobblestone world storage v1 design

Status: design contract only. Persistence implementation begins after chunk load/unload/pinning semantics are explicit.

## Goals

The storage format must serve Cobblestone's native world representation rather than copy Anvil, LevelDB, or protocol-84 packet bytes. It must provide:

- bounded recovery after process or machine failure;
- fast native asynchronous save/load without routing chunk planes through PHP;
- immutable snapshot input so gameplay never waits on compression or disk I/O;
- compact fixed-target block/light representation;
- independent terrain/light revisions;
- sparse extra block data;
- versioned future extension space for block entities and additional world data;
- deterministic corruption detection;
- region-local compaction; and
- no coupling between disk topology and execution-region scheduling.

It is not intended to be a general Bedrock world interchange format.

## Directory layout

A world directory contains:

- `world.cwm` — atomic world metadata;
- `regions/r.<rx>.<rz>.cwr` — chunk region containers;
- temporary `*.tmp` files only during atomic metadata replacement or region compaction.

Storage regions are 16×16 chunks. Storage region coordinates are floor-divided independently from the current 8×8 execution regions. Changing runtime scheduling therefore never changes disk addresses.

## World metadata

`world.cwm` starts with a fixed binary envelope:

- magic `CBWM`;
- format version;
- envelope length;
- metadata generation;
- payload length;
- CRC32C of the payload.

The version-1 payload stores canonical semantic world metadata: world UUID, name, seed, fixed-target marker (0.15.10 / protocol 84 / RakNet 8), generator ID and opaque versioned generator settings, spawn, time, and time-running state.

Metadata updates use write-temp, fdatasync, atomic rename, then parent-directory fsync. No in-place metadata mutation is authoritative.

## Region container

A `.cwr` file begins with:

1. a small immutable region header containing magic `CBRG`, format version, storage-region coordinates, and world UUID;
2. two fixed 16 KiB index pages A/B; and
3. an append-only record area.

Each index page has its own generation and CRC32C. It contains 256 fixed entries, one per local chunk. An entry records:

- record offset;
- stored record length;
- record/payload checksum;
- terrain revision; and
- light revision.

On open, the reader validates both pages and chooses the valid page with the highest generation. A torn or partially written newer page is ignored.

## Commit protocol

Saving one or more chunks in a region is copy-on-write:

1. append complete immutable chunk records;
2. fdatasync the region file;
3. build the inactive index page from the current index plus the new record locations;
4. write the entire inactive index page with generation + 1 and checksum;
5. fdatasync again.

There is no fragile in-place pointer flip. Recovery simply selects the highest valid index generation. Old records remain valid until compaction.

## Chunk record

Every appended record has a checksummed fixed header containing:

- magic `CBCH`;
- chunk-record version;
- absolute chunk X/Z;
- terrain revision;
- light revision;
- lifecycle flags (generated, populated, light-populated);
- compression method;
- uncompressed payload length;
- stored payload length;
- payload CRC32C; and
- header CRC32C.

Records are immutable once published by an index page.

## Semantic payload v1

The core payload is deliberately not a FullChunkData packet. It uses the fixed-target semantic planes directly:

- 32,768 block-ID bytes in Y/Z/X order;
- 16,384 packed block-data nibble bytes;
- 16,384 packed sky-light nibble bytes;
- 16,384 packed block-light nibble bytes;
- 256 biome-ID bytes in Z/X order;
- 256 height-map bytes;
- little-endian u32 sparse extra-data count;
- sorted entries of `u16 linear_block_index + u16 value`; and
- zero or more versioned extension sections.

An extension section is `tag:u16, version:u16, length:u32, payload`. Unknown optional tags can be skipped. Mandatory future schema changes require a new record/payload version.

The native in-memory state remains `u16 BlockStateId`. The storage worker repacks/unpacks the ID+nibble state planes during save/load.

Measured whole-chunk repack cost is about 60.7 us for u16 → ID+nibble and 75.3 us for ID+nibble → u16. That cost is small relative to compression/I/O and buys a better worst-case disk representation.

With zstd level 1 on the state plane alone:

| Terrain | u16 raw → zstd | ID+nibble raw → zstd |
| --- | ---: | ---: |
| default Flat | 65,536 → 35 B | 49,152 → 34 B |
| random 12-bit states | 65,536 → 56,047 B | 49,152 → 49,166 B |

The compact semantic representation therefore avoids about 12% compressed overhead on high-entropy state data without materially affecting Flat-world compression.

## Compression

Version 1 should support:

- `none`; and
- `zstd`, with level 1 as the normal save path.

The complete semantic payload is compressed as one frame. Network zlib settings are unrelated to disk compression and must not leak into storage.

Compression choice remains a record field so later versions can change policy without a world migration.

## Native asynchronous persistence

Persistence runs below the Zend boundary.

A save request captures an immutable native chunk snapshot/Arc plus lifecycle metadata and queues a bounded Rust storage job. Compression, checksumming, file writes, fsync, and index publication happen off the PHP owner runtime.

Completion records which terrain/light revisions reached stable storage. If the live chunk advanced while the save was running, the newer revision remains dirty and is scheduled again. PHP does not receive or rebuild chunk planes.

The network change journal is not reused for persistence. Network delivery cursors and durable-save state have different retention and failure semantics.

## Load and unload contract

The residency contract is now explicit and native-authoritative:

- every resident native chunk carries generated/populated/light-populated lifecycle flags, terrain/light revisions, persisted terrain/light/lifecycle watermarks, and a runtime-only pin count;
- a newly generated chunk starts dirty because it has no persisted watermark;
- an imported storage record enters residency clean at the record's exact revisions/lifecycle flags;
- immutable native snapshots capture terrain revision, light revision, lifecycle flags, and the chunk data Arc atomically;
- persistence completion advances persisted watermarks monotonically to the exact snapshot that reached stable storage; if live revisions or lifecycle advanced meanwhile, the chunk remains dirty;
- `ResidentChunkHandle` pins the chunk for its lifetime, and native session views pin every streamed chunk until disconnect/runtime shutdown;
- safe unload returns Missing, Pinned, Dirty, or Unloaded. Only a clean, zero-pin chunk can leave native residency;
- protocol-84 cached FullChunkData is discarded when residency eviction succeeds;
- world destruction is rejected while any chunk pins remain active;
- PHP owner-runtime generation permits only one in-flight load/generation operation for a chunk key and rejects reentrant duplicate creation.

`MainChunkSource::remove()` remains a compatibility wrapper around safe unload: it returns the former PHP facade only when unload actually succeeds. Runtime code should use `unload()` and inspect the status rather than interpreting a missing return value.

The remaining persistence-specific load work is asynchronous duplicate-load collapse: when disk loading is introduced, the native storage layer should keep one in-flight load future per chunk coordinate and let all requesters join that result rather than scheduling duplicate reads. Generation remains the fallback only after the storage layer reports the chunk absent.

For unload with persistence, the intended path is snapshot handoff rather than blocking the gameplay runtime on disk: capture the immutable native snapshot, pin/retain that snapshot in the bounded storage job, publish it durably, advance persisted watermarks on completion, and evict only when the live chunk is still clean and unpinned. Shutdown must drain or explicitly fail accepted save jobs before destroying the world store. Exact save/load queue capacities and backpressure thresholds remain implementation-time measured constants.

## Compaction

Append-only regions accumulate dead records. A native maintenance task rewrites live records to a sibling temporary region, writes fresh dual indexes, fdatasyncs, atomically renames, and fsyncs the directory.

Initial trigger policy should be measured, with a starting candidate of either >50% dead bytes or >64 MiB reclaimable garbage. Compaction never runs on the PHP gameplay thread.

## Corruption policy

Checksums detect corruption; they do not authenticate data.

- invalid newest index page → fall back to the other valid page;
- invalid referenced record → report the chunk/region as corrupt;
- both invalid indexes → region-open failure;
- no silent replacement of corrupt data with an empty chunk;
- optional repair tooling may later scan append records and reconstruct an index, but repair is never automatic gameplay behavior.

## Implementation sequencing

Live synchronization and chunk residency now fix the authoritative revision/snapshot, pinning, lifecycle, dirty-watermark, and safe-unload contracts. Persistence no longer needs to invent those semantics.

The next storage milestone can implement the native async save/load mechanism directly against this format: region/index I/O, zstd/CRC32C record encoding, bounded storage jobs, one in-flight load per chunk, persisted-watermark completion, and clean eviction. Compaction can follow once real save workloads provide dead-byte measurements.
