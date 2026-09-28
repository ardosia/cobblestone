<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Encodes and decodes the private PHP/native storage projections.
 *
 * @internal
 */
final class NativeWorldStorageProjection
{
    /**
     * @param list<ChunkPos> $positions
     */
    public static function encodeLoadBatch(array $positions): string
    {
        if (count($positions) > 4096) {
            throw new \ValueError('native world storage load batch cannot exceed 4096 chunks');
        }

        $projection = [pack('V', count($positions))];
        foreach ($positions as $position) {
            if (!$position instanceof ChunkPos) {
                throw new \TypeError('native world storage load batch expects ChunkPos values');
            }
            $projection[] = pack('V', $position->x & 0xffffffff);
            $projection[] = pack('V', $position->z & 0xffffffff);
        }
        return implode('', $projection);
    }

    public static function loadBatchCount(string $projection): int
    {
        if (strlen($projection) < 4) {
            throw new \ValueError('native world storage load batch is truncated');
        }

        $header = unpack('Vcount', substr($projection, 0, 4));
        $count = (int) ($header['count'] ?? -1);
        if ($count < 0 || $count > 4096 || strlen($projection) !== 4 + ($count * 8)) {
            throw new \ValueError('native world storage load batch has invalid length');
        }

        return $count;
    }

    /**
     * @param array<int, mixed> $values
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
    public static function decodeTick(array $values): array
    {
        if (count($values) !== 12) {
            throw new \UnexpectedValueException('native world storage tick projection has wrong width');
        }

        return [
            'completed' => (int) $values[0],
            'scheduled' => (int) $values[1],
            'in_flight' => (int) $values[2],
            'metadata_generation' => (int) $values[3],
            'load_completed' => (int) $values[4],
            'load_in_flight' => (int) $values[5],
            'load_missing' => (int) $values[6],
            'compaction_completed' => (int) $values[7],
            'compaction_failed' => (int) $values[8],
            'compaction_scheduled' => (int) $values[9],
            'compaction_in_flight' => (int) $values[10],
            'compaction_queued' => (int) $values[11],
        ];
    }

    /**
     * @param array<int, mixed> $values
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
    public static function decodeStats(array $values): array
    {
        if (count($values) !== 10) {
            throw new \UnexpectedValueException('native world storage stats projection has wrong width');
        }

        return [
            'save_bytes_appended' => (int) $values[0],
            'regions_observed' => (int) $values[1],
            'record_bytes' => (int) $values[2],
            'live_bytes' => (int) $values[3],
            'dead_bytes' => (int) $values[4],
            'compactions_completed' => (int) $values[5],
            'compactions_failed' => (int) $values[6],
            'compaction_bytes_reclaimed' => (int) $values[7],
            'compaction_blocked_regions' => (int) $values[8],
            'compaction_last_error' => (string) $values[9],
        ];
    }
}
