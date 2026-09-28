# Cobblestone world storage v1 design

Status: v1 world metadata, binary region/chunk format, bounded native async save/load orchestration, and native WorldStore save/load attachment implemented; production chunk-acquisition/server composition and compaction remain pending.

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

`world.cwm` is now implemented and frozen for storage format v1. It begins with a 32-byte little-endian envelope:

- bytes 0..3: magic `CBWM`;
- 4..5: storage format version `u16 = 1`;
- 6..7: envelope length `u16 = 32`;
- 8..15: metadata generation `u64`, starting at 1;
- 16..19: semantic payload length `u32`;
- 20..23: CRC32C of the semantic payload;
- 24..27: CRC32C of envelope bytes 0..23; and
- 28..31: reserved zero bytes.

The version-1 semantic payload has a fixed 76-byte prefix followed by UTF-8 world-name bytes and opaque generator-settings bytes:

- 0..1: payload version `u16 = 1`;
- 2..4: fixed target version bytes `0, 15, 10`;
- 5: time-running flag `u8` restricted to 0/1;
- 6..7: reserved zero bytes;
- 8..11: game protocol `u32 = 84`;
- 12..15: RakNet protocol `u32 = 8`;
- 16..31: raw 16-byte world UUID;
- 32..39: world seed `i64`;
- 40..43: generator ID `u32`;
- 44..45: generator-settings schema version `u16`;
- 46..47: reserved zero bytes;
- 48..51 / 52..55 / 56..59: spawn X/Y/Z `i32`;
- 60..67: world time `i64`;
- 68..71: UTF-8 world-name length `u32`;
- 72..75: opaque generator-settings length `u32`; then
- world-name bytes followed immediately by generator-settings bytes.

Metadata payloads are bounded to 1 MiB, names to 4 KiB, generator settings to 512 KiB, and fixed-target spawn Y to 0..127. Decode rejects checksum failures, nonzero reserved bytes, malformed UTF-8, size mismatches, unsupported versions, or a fixed-target marker other than 0.15.10 / protocol 84 / RakNet 8.

`WorldDirectory` owns the storage root, `world.cwm`, and `regions/` path. First creation publishes a fully synced temporary metadata file through a no-overwrite hard-link step, then fsyncs the parent directory. Metadata replacement increments generation, writes a unique temporary file, `sync_data`s it, atomically renames over `world.cwm`, and fsyncs the parent directory. Replacement cannot change the world UUID. No in-place metadata mutation is authoritative.

## Region container

A `.cwr` file begins with:

1. a 64-byte immutable region header containing magic `CBRG`, format version, storage-region coordinates, and world UUID;
2. two fixed 16 KiB index pages A/B; and
3. an append-only record area beginning at byte 32,832.

All integer fields are little-endian. The v1 64-byte region header is frozen as:

- bytes 0..3: `CBRG`;
- 4..5: format version `u16`;
- 6..7: header length `u16 = 64`;
- 8..11 / 12..15: signed storage-region X/Z `i32`;
- 16..31: raw 16-byte world UUID;
- 32..33: storage-region edge `u16 = 16`;
- 34..35: index-page length `u16 = 16384`;
- 36: index-page count `u8 = 2`;
- 37..39: reserved zero bytes;
- 40..47: record-area offset `u64 = 32832`;
- 48..59: reserved zero bytes; and
- 60..63: CRC32C of bytes 0..59.

Each index page has its own generation and CRC32C. Its 32-byte header is frozen as bytes 0..3 `CBIX`, 4..5 format version `u16`, 6..7 page length `u16 = 16384`, 8..15 generation `u64`, 16..19 / 20..23 region X/Z `i32`, 24..27 entry count `u32 = 256`, and 28..31 reserved zero bytes. Entries begin at byte 32: 256 fixed 40-byte slots occupy bytes 32..10271, bytes 10272..16379 are zero padding, and bytes 16380..16383 store CRC32C over bytes 0..16379. Each entry records:

- record offset `u64`;
- complete stored record length `u32`;
- CRC32C of the complete stored record `u32`;
- terrain revision `u64`;
- light revision `u64`; and
- eight reserved zero bytes.

An all-zero entry is absent. On open, the reader validates both pages and chooses the valid page with the highest generation. A torn or partially written newer page is ignored.

## Commit protocol

Saving one or more chunks in a region is copy-on-write:

1. append complete immutable chunk records;
2. fdatasync the region file;
3. build the inactive index page from the current index plus the new record locations;
4. write the entire inactive index page with generation + 1 and checksum;
5. fdatasync again.

There is no fragile in-place pointer flip. Recovery simply selects the highest valid index generation. Old records remain valid until compaction.

## Chunk record

Every appended record has a checksummed fixed 64-byte header. The v1 layout is frozen as:

- bytes 0..3: `CBCH`;
- 4..5: chunk-record version `u16 = 1`;
- 6..7: header length `u16 = 64`;
- 8..11 / 12..15: absolute chunk X/Z `i32`;
- 16..23: terrain revision `u64`;
- 24..31: light revision `u64`;
- 32: lifecycle flags `u8` (generated, populated, light-populated);
- 33: stored compression method `u8` (`0 = none`, `1 = zstd`);
- 34..35: semantic payload version `u16 = 1`;
- 36..39: uncompressed payload length `u32`;
- 40..43: stored payload length `u32`;
- 44..47: CRC32C of the uncompressed semantic payload;
- 48..59: reserved zero bytes; and
- 60..63: CRC32C of header bytes 0..59.

The index entry additionally stores CRC32C over the complete header + stored payload, so recovery detects both record corruption and mismatched index targets. Records are immutable once published by an index page.

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

Version 1 stores only two wire methods:

- `none`; and
- `zstd` level 1.

The normal save policy is adaptive rather than blindly compressed. The storage worker tries zstd level 1 and keeps it only when it saves at least 4 KiB versus the canonical uncompressed semantic payload; otherwise the record is stored as `none`. The policy is not an on-disk compression value, so future threshold changes do not require a world migration.

Release measurements on the development host for complete v1 records (64-byte record header included) were:

| Terrain | none bytes / encode | zstd-1 bytes / encode | adaptive selection |
| --- | ---: | ---: | --- |
| default Flat | 82,500 / ~135 µs | 111 / ~154 µs | zstd, ~158 µs |
| high-entropy semantic planes | 82,500 / ~139 µs | 82,509 / ~208 µs | none, ~207 µs |

A synchronous single-record region commit using the adaptive policy measured about 179 µs for Flat and 283 µs for the high-entropy case on the development host, including the two `sync_data` durability barriers. These are implementation-host measurements, not guaranteed device latency targets.

The complete semantic payload is compressed as one frame. Network zlib settings are unrelated to disk compression and must not leak into storage.

## Native asynchronous persistence

Persistence runs below the Zend boundary.

The native save side is implemented as a region-sharded bounded worker service. A save submission transfers one immutable native `ChunkSnapshot`/Arc to Rust; the worker route is derived from the 16×16 storage-region coordinate, so every chunk in one region is serialized through the same worker while unrelated regions can commit concurrently. Each worker lazily owns/caches its `RegionFile` handles.

The service accepts 1..32 workers and bounded per-worker command plus shared completion capacities. The current defaults are one worker, 1,024 queued commands per worker, and 1,024 completion slots. `try_save()` never blocks the owner runtime: a full queue returns the original snapshot as explicit backpressure, while a closed worker returns the snapshot as a closed error.

Compression, checksumming, append writes, both durability barriers, and inactive-index publication happen on the storage worker. Each completion carries the exact chunk coordinate, terrain revision, light revision, lifecycle flags, region generation, and bytes appended for the immutable snapshot that reached stable storage. Worker panics are contained per job and surfaced as failed completions.

Shutdown closes command senders, drains every accepted save to a completion, and only then joins the workers. This keeps accepted durable work from disappearing merely because the server is stopping. Persisted watermarks are intentionally not advanced inside the storage crate. The native PHP extension now owns that composition seam: one coarse storage tick polls save receipts, applies their exact terrain/light/lifecycle watermarks directly to the live `WorldStore`, and schedules additional dirty immutable snapshots without exposing chunk planes to PHP. If the live chunk advanced while a save was running, it remains dirty and is selected again.

Release measurement on the development host for 64 default-Flat snapshots spread across 16 storage regions measured:

| save workers | total durable time | effective per chunk | throughput |
| ---: | ---: | ---: | ---: |
| 1 | ~14.50 ms | ~226.5 µs | ~4.4k chunks/s |
| 2 | ~7.47 ms | ~116.6 µs | ~8.6k chunks/s |
| 4 | ~3.98 ms | ~62.2 µs | ~16.1k chunks/s |

These numbers include region record encoding and durable region/index commits on the development machine. They justify region-sharded save workers, but they are not device latency guarantees and do not imply that the production default should always be four workers.

The async load side uses the same bounded worker model but deduplicates by exact chunk coordinate before queue submission. The first requester receives `Queued`; later requests for the same in-flight coordinate receive `Joined` and do not schedule another disk read. The coordinate leaves the in-flight set only when its completion is consumed or shutdown drains it.

Load workers deliberately reopen the region for each newly queued chunk load rather than caching a `RegionFile`. Save workers keep mutable cached indexes; an independent cached reader could otherwise retain a stale index after a later durable save. A missing region or empty index slot completes as `Missing` and never creates a region file. Corruption and world/region mismatches surface as failed completions rather than falling through to generation.

Release measurement on the development host for 64 default-Flat chunks spread across 16 storage regions, with the region data already in the OS page cache, measured:

| load workers | total load time | effective per chunk | throughput |
| ---: | ---: | ---: | ---: |
| 1 | ~12.55 ms | ~196.0 µs | ~5.1k chunks/s |
| 2 | ~6.41 ms | ~100.2 µs | ~10.0k chunks/s |
| 4 | ~3.42 ms | ~53.4 µs | ~18.7k chunks/s |

These figures include region reopen/index selection, indexed-record CRC32C validation, zstd decode, payload validation, and semantic `ChunkImport` reconstruction. They are warm-cache development-host measurements, not storage-device latency guarantees.

The network change journal is not reused for persistence. Network delivery cursors and durable-save/load state have different retention and failure semantics.

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

Persistence-specific duplicate-load collapse is implemented end-to-end in the native world bridge: exactly one disk request may be in flight per chunk coordinate, and duplicate requesters join that operation. The coarse request state is `Resident`, `Queued`, `Joined`, or durable `Missing`. Storage ticks consume load completions below Zend and import decoded `ChunkImport` records directly into `WorldStore` with an atomic import-if-absent rule, so a late disk completion cannot overwrite live residency. Imported records enter clean at their durable terrain/light/lifecycle revisions. A failed/corrupt load makes the storage tick fail; it is never converted into generation. Only a durable `Missing` result is permission to generate, and native generation clears the remembered miss and starts the new chunk dirty.

Save-side snapshot handoff is now implemented: the extension scans dirty native residency below Zend, submits immutable snapshots to the bounded save service, advances persisted watermarks only after durable receipts, and leaves newer live revisions dirty. Explicit world destruction additionally schedules every still-unsaved dirty snapshot and waits for completion before removing the native world handle. Safe eviction remains separately gated by clean + unpinned state; automatic eviction policy belongs with the upcoming load/residency integration rather than the save worker.

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

Live synchronization and chunk residency fix the authoritative revision/snapshot, pinning, lifecycle, dirty-watermark, and safe-unload contracts. The `cobblestone-storage` crate implements the v1 chunk-record codec, bounded decompression, CRC32C validation, adaptive zstd policy, 16×16 region addressing, dual-index recovery, append + `sync_data` + inactive-index commit ordering, parent-directory fsync on first region creation, bounded region-sharded async saves, and deduplicated bounded async loads. Tests cover torn-index recovery, exact save receipts, save shutdown draining, durable misses without file creation, and one in-flight disk read per chunk.

The next bounded milestone is production chunk acquisition/server composition: persistent worlds must request/join native loads without blocking the PHP owner runtime, expose a Fiber-friendly resident-or-miss continuation, generate only after durable `Missing`, and call one coarse native storage tick from the server loop. After that, clean unpinned eviction policy can be enabled around player/view residency. Compaction follows once real save workloads provide dead-byte measurements.
