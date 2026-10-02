<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Native persistence operations for one live world-store handle.
 *
 * @internal
 */
final class NativeWorldStorage
{
    private function __construct()
    {
    }

    public static function attach(
        int $handle,
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
        if ($root === '') {
            throw new \ValueError('native world storage root cannot be empty');
        }
        if ($saveWorkers <= 0 || $saveWorkers > 32) {
            throw new \ValueError('native world save worker count must be in range 1..32');
        }
        if ($loadWorkers <= 0 || $loadWorkers > 32) {
            throw new \ValueError('native world load worker count must be in range 1..32');
        }
        if ($compactionMinDeadBytes < 0) {
            throw new \ValueError('native world compaction minimum dead bytes must be nonnegative');
        }
        if ($compactionMinDeadPercent < 0 || $compactionMinDeadPercent > 100) {
            throw new \ValueError('native world compaction minimum dead percent must be in range 0..100');
        }

        $createUuid ??= random_bytes(16);
        if (strlen($createUuid) !== 16) {
            throw new \ValueError('native world creation UUID must contain exactly 16 bytes');
        }

        $values = cobblestone_world_storage_attach(
            $handle,
            $root,
            [
                $createUuid,
                $createName,
                $createSeed,
                $createGeneratorId,
                $createGeneratorSettingsVersion,
                $createGeneratorSettings,
                $createSpawn->x,
                $createSpawn->y,
                $createSpawn->z,
                $createTime,
                $createTimeRunning,
            ],
            $saveWorkers,
            $loadWorkers,
            $compactionMinDeadBytes,
            $compactionMinDeadPercent,
        );

        return NativeWorldMetadata::fromNative($values);
    }

    public static function prepareLoadBatch(int $handle, string $projection): string
    {
        $count = NativeWorldStorageProjection::loadBatchCount($projection);
        $statuses = cobblestone_world_storage_prepare_loads($handle, $projection);
        if (strlen($statuses) !== $count) {
            throw new \UnexpectedValueException('native world storage load batch status width mismatch');
        }

        return $statuses;
    }

    public static function requestLoad(int $handle, ChunkPos $position): NativeChunkLoadStatus
    {
        return NativeChunkLoadStatus::from(
            cobblestone_world_storage_request_load(
                $handle,
                $position->x,
                $position->z,
            ),
        );
    }

    /**
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
    public static function tick(int $handle, int $budget = 64): array
    {
        if ($budget <= 0 || $budget > 4096) {
            throw new \ValueError('native world storage tick budget must be in range 1..4096');
        }

        return NativeWorldStorageProjection::decodeTick(
            cobblestone_world_storage_tick($handle, $budget),
        );
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
    public static function stats(int $handle): array
    {
        return NativeWorldStorageProjection::decodeStats(
            cobblestone_world_storage_stats($handle),
        );
    }

    public static function flush(int $handle): void
    {
        cobblestone_world_storage_flush($handle);
    }
}
