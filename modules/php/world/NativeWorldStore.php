<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Internal owner-runtime bridge to the Rust world store.
 *
 * Plugin-facing APIs stay on World/Chunk; this class keeps the native handle and FFI names out of
 * normal gameplay code.
 *
 * @internal
 */
final class NativeWorldStore
{
    private ?int $handle;
    private bool $storageAttached = false;

    private function __construct(int $handle)
    {
        $this->handle = $handle;
    }

    public static function available(): bool
    {
        if (!extension_loaded('cobblestone_core_php')) {
            return false;
        }

        foreach ([
            'cobblestone_world_create',
            'cobblestone_world_storage_attach',
            'cobblestone_world_storage_prepare_loads',
            'cobblestone_world_storage_request_load',
            'cobblestone_world_storage_tick',
            'cobblestone_world_storage_stats',
            'cobblestone_world_storage_flush',
            'cobblestone_world_destroy',
            'cobblestone_world_ensure_chunk',
            'cobblestone_world_lifecycle_flags',
            'cobblestone_world_set_lifecycle_flags',
            'cobblestone_world_pin_chunk',
            'cobblestone_world_unpin_chunk',
            'cobblestone_world_chunk_pin_count',
            'cobblestone_world_chunk_dirty',
            'cobblestone_world_mark_persisted',
            'cobblestone_world_try_evict_chunk',
            'cobblestone_world_terrain_revision',
            'cobblestone_world_light_revision',
            'cobblestone_world_commit_terrain_revision',
            'cobblestone_world_commit_light_revision',
            'cobblestone_world_block_state',
            'cobblestone_world_set_block_state',
            'cobblestone_world_fill_layers',
            'cobblestone_world_biome',
            'cobblestone_world_set_biome',
            'cobblestone_world_fill_biome',
            'cobblestone_world_sky_light',
            'cobblestone_world_set_sky_light',
            'cobblestone_world_fill_sky_light_from',
            'cobblestone_world_block_light',
            'cobblestone_world_set_block_light',
            'cobblestone_world_height_map',
            'cobblestone_world_recalculate_height_map',
            'cobblestone_world_block_extra_data',
            'cobblestone_world_set_block_extra_data',
            'cobblestone_world_apply_patch',
            'cobblestone_world_snapshot',
        ] as $function) {
            if (!\function_exists($function)) {
                return false;
            }
        }

        return true;
    }

    public static function create(): self
    {
        if (!self::available()) {
            throw new \RuntimeException('cobblestone_core_php world store is unavailable or stale');
        }

        return new self(cobblestone_world_create());
    }

    public function handle(): int
    {
        return $this->requireHandle();
    }

    public function attachStorage(
        string $root,
        string $createName,
        int $createSeed,
        int $createGeneratorId,
        int $createGeneratorSettingsVersion,
        string $createGeneratorSettings,
        BlockPos $createSpawn,
        int $createTime = 0,
        bool $createTimeRunning = true,
        int $saveWorkers = 2,
        int $loadWorkers = 2,
        int $compactionMinDeadBytes = 0,
        int $compactionMinDeadPercent = 0,
        ?string $createUuid = null,
    ): NativeWorldMetadata {
        $metadata = NativeWorldStorage::attach(
            $this->requireHandle(),
            $root,
            $createName,
            $createSeed,
            $createGeneratorId,
            $createGeneratorSettingsVersion,
            $createGeneratorSettings,
            $createSpawn,
            $createTime,
            $createTimeRunning,
            $saveWorkers,
            $loadWorkers,
            $compactionMinDeadBytes,
            $compactionMinDeadPercent,
            $createUuid,
        );
        $this->storageAttached = true;

        return $metadata;
    }

    public function hasStorage(): bool
    {
        return $this->storageAttached;
    }

    /**
     * Builds the reusable private PHP/native load projection once for repeated polling.
     *
     * @param list<ChunkPos> $positions
     */
    public static function encodeStorageLoadBatch(array $positions): string
    {
        return NativeWorldStorageProjection::encodeLoadBatch($positions);
    }

    /**
     * Hot path: one FFI crossing for the whole pre-encoded chunk set.
     *
     * Returns one raw NativeChunkLoadStatus byte per encoded coordinate.
     */
    public function prepareStorageLoadBatch(string $projection): string
    {
        return NativeWorldStorage::prepareLoadBatch($this->requireHandle(), $projection);
    }


    public function requestStorageLoad(ChunkPos $position): NativeChunkLoadStatus
    {
        return NativeWorldStorage::requestLoad($this->requireHandle(), $position);
    }

    /**
     * Polls save completions and schedules dirty immutable snapshots entirely in Rust.
     *
     * @return array{
     *   completed: int,
     *   scheduled: int,
     *   in_flight: int,
     *   metadata_generation: int,
     *   load_completed: int,
     *   load_in_flight: int,
     *   load_missing: int,
     *   compaction_completed: int,
     *   compaction_failed: int,
     *   compaction_scheduled: int,
     *   compaction_in_flight: int,
     *   compaction_queued: int
     * }
     */
    public function storageTick(int $budget = 64): array
    {
        return NativeWorldStorage::tick($this->requireHandle(), $budget);
    }

    /**
     * @return array{
     *   save_bytes_appended: int,
     *   regions_observed: int,
     *   record_bytes: int,
     *   live_bytes: int,
     *   dead_bytes: int,
     *   compactions_completed: int,
     *   compactions_failed: int,
     *   compaction_bytes_reclaimed: int,
     *   compaction_blocked_regions: int,
     *   compaction_last_error: string
     * }
     */
    public function storageStats(): array
    {
        return NativeWorldStorage::stats($this->requireHandle());
    }

    public function flushStorage(): void
    {
        if (!$this->storageAttached) {
            return;
        }

        NativeWorldStorage::flush($this->requireHandle());
    }

    public function ensureChunk(ChunkPos $position, BiomeId $biome): bool
    {
        return cobblestone_world_ensure_chunk(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $biome->value,
        );
    }

    public function lifecycleFlags(ChunkPos $position): int
    {
        return NativeChunkResidency::lifecycleFlags($this->requireHandle(), $position);
    }

    public function setLifecycleFlags(ChunkPos $position, int $flags): void
    {
        NativeChunkResidency::setLifecycleFlags($this->requireHandle(), $position, $flags);
    }

    public function pinChunk(ChunkPos $position): int
    {
        return NativeChunkResidency::pin($this->requireHandle(), $position);
    }

    public function unpinChunk(ChunkPos $position): int
    {
        return NativeChunkResidency::unpin($this->requireHandle(), $position);
    }

    public function chunkPinCount(ChunkPos $position): int
    {
        return NativeChunkResidency::pinCount($this->requireHandle(), $position);
    }

    public function chunkDirty(ChunkPos $position): bool
    {
        return NativeChunkResidency::dirty($this->requireHandle(), $position);
    }

    public function markPersisted(
        ChunkPos $position,
        int $terrainRevision,
        int $lightRevision,
        int $lifecycleFlags,
    ): void {
        NativeChunkResidency::markPersisted(
            $this->requireHandle(),
            $position,
            $terrainRevision,
            $lightRevision,
            $lifecycleFlags,
        );
    }

    /** @return 0|1|2|3 0=missing, 1=pinned, 2=dirty, 3=evicted */
    public function tryEvictChunk(ChunkPos $position): int
    {
        return NativeChunkResidency::tryEvict($this->requireHandle(), $position);
    }

    public function terrainRevision(ChunkPos $position): int
    {
        return NativeChunkRevision::terrain($this->requireHandle(), $position);
    }

    public function lightRevision(ChunkPos $position): int
    {
        return NativeChunkRevision::light($this->requireHandle(), $position);
    }

    public function commitTerrainRevision(ChunkPos $position, int $expected, int $next): void
    {
        NativeChunkRevision::commitTerrain($this->requireHandle(), $position, $expected, $next);
    }

    public function commitLightRevision(ChunkPos $position, int $expected, int $next): void
    {
        NativeChunkRevision::commitLight($this->requireHandle(), $position, $expected, $next);
    }

    public function blockStateId(ChunkPos $position, int $x, int $y, int $z): int
    {
        return cobblestone_world_block_state(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $x,
            $y,
            $z,
        );
    }

    public function setBlockStateId(ChunkPos $position, int $x, int $y, int $z, int $stateId): int
    {
        return cobblestone_world_set_block_state(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $x,
            $y,
            $z,
            $stateId,
        );
    }

    public function fillLayers(ChunkPos $position, int $startY, int $count, int $stateId): void
    {
        cobblestone_world_fill_layers(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $startY,
            $count,
            $stateId,
        );
    }

    public function biome(ChunkPos $position, int $x, int $z): int
    {
        return cobblestone_world_biome(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $x,
            $z,
        );
    }

    public function setBiome(ChunkPos $position, int $x, int $z, int $biome): int
    {
        return cobblestone_world_set_biome(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $x,
            $z,
            $biome,
        );
    }

    public function skyLight(ChunkPos $position, int $x, int $y, int $z): int
    {
        return cobblestone_world_sky_light(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $x,
            $y,
            $z,
        );
    }

    public function setSkyLight(ChunkPos $position, int $x, int $y, int $z, int $level): int
    {
        return cobblestone_world_set_sky_light(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $x,
            $y,
            $z,
            $level,
        );
    }

    public function fillSkyLightFrom(ChunkPos $position, int $y, int $level): void
    {
        cobblestone_world_fill_sky_light_from(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $y,
            $level,
        );
    }

    public function blockLight(ChunkPos $position, int $x, int $y, int $z): int
    {
        return cobblestone_world_block_light(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $x,
            $y,
            $z,
        );
    }

    public function setBlockLight(ChunkPos $position, int $x, int $y, int $z, int $level): int
    {
        return cobblestone_world_set_block_light(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $x,
            $y,
            $z,
            $level,
        );
    }

    public function heightMap(ChunkPos $position, int $x, int $z): int
    {
        return cobblestone_world_height_map(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $x,
            $z,
        );
    }

    public function recalculateHeightMap(ChunkPos $position): void
    {
        cobblestone_world_recalculate_height_map(
            $this->requireHandle(),
            $position->x,
            $position->z,
        );
    }

    public function blockExtraData(ChunkPos $position, int $x, int $y, int $z): int
    {
        return cobblestone_world_block_extra_data(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $x,
            $y,
            $z,
        );
    }

    public function setBlockExtraData(
        ChunkPos $position,
        int $x,
        int $y,
        int $z,
        int $data,
    ): int {
        return cobblestone_world_set_block_extra_data(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $x,
            $y,
            $z,
            $data,
        );
    }



    /**
     * @param array<int, int> $blocks linear block index => scalar state id
     * @param array<int, BiomeId> $biomes column index => biome
     * @param array<int, int> $extraData linear block index => extra data
     * @param array<int, int> $skyLight linear block index => light
     * @param array<int, int> $blockLight linear block index => light
     */
    public function applyPatch(
        ChunkPos $position,
        int $expectedTerrainRevision,
        int $nextTerrainRevision,
        int $expectedLightRevision,
        int $nextLightRevision,
        array $blocks,
        array $biomes,
        array $extraData,
        array $skyLight,
        array $blockLight,
    ): void {
        $payload = pack('P4', $expectedTerrainRevision, $nextTerrainRevision, $expectedLightRevision, $nextLightRevision)
            . pack('V5', count($blocks), count($biomes), count($extraData), count($skyLight), count($blockLight));

        foreach ($blocks as $index => $stateId) {
            $payload .= pack('vv', $index, $stateId);
        }
        foreach ($biomes as $index => $biome) {
            $payload .= pack('CC', $index, $biome->value);
        }
        foreach ($extraData as $index => $value) {
            $payload .= pack('vv', $index, $value);
        }
        foreach ($skyLight as $index => $level) {
            $payload .= pack('vC', $index, $level);
        }
        foreach ($blockLight as $index => $level) {
            $payload .= pack('vC', $index, $level);
        }

        cobblestone_world_apply_patch(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $payload,
        );
    }

    /**
     * Materializes one immutable chunk projection for compatibility/debug consumers.
     *
     * Layout: terrainRevision(u64le), lightRevision(u64le), blockIds, blockData, skyLight,
     * blockLight, biomes, heightMap, extraCount(u32le), then extraData key/value u16le pairs.
     */
    public function snapshotProjection(ChunkPos $position): string
    {
        return cobblestone_world_snapshot(
            $this->requireHandle(),
            $position->x,
            $position->z,
        );
    }

    public function destroy(): void
    {
        if ($this->handle === null) {
            return;
        }

        $handle = $this->handle;
        cobblestone_world_destroy($handle);
        $this->handle = null;
        $this->storageAttached = false;
    }

    public function __destruct()
    {
        if ($this->handle !== null && \function_exists('cobblestone_world_destroy')) {
            try {
                cobblestone_world_destroy($this->handle);
            } catch (\Throwable) {
                // Extension/module shutdown owns the final cleanup fallback.
            }
            $this->handle = null;
        }
    }

    private function requireHandle(): int
    {
        return $this->handle
            ?? throw new \LogicException('native world store has already been destroyed');
    }
}
