<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Native residency, lifecycle, and persistence-watermark operations for one world-store handle.
 *
 * @internal
 */
final class NativeChunkResidency
{
    private function __construct()
    {
    }

    public static function lifecycleFlags(int $handle, ChunkPos $position): int
    {
        return cobblestone_world_lifecycle_flags(
            $handle,
            $position->x,
            $position->z,
        );
    }

    public static function setLifecycleFlags(int $handle, ChunkPos $position, int $flags): void
    {
        cobblestone_world_set_lifecycle_flags(
            $handle,
            $position->x,
            $position->z,
            $flags,
        );
    }

    public static function pin(int $handle, ChunkPos $position): int
    {
        return cobblestone_world_pin_chunk(
            $handle,
            $position->x,
            $position->z,
        );
    }
    public static function unpin(int $handle, ChunkPos $position): int
    {
        return cobblestone_world_unpin_chunk(
            $handle,
            $position->x,
            $position->z,
        );
    }

    public static function pinCount(int $handle, ChunkPos $position): int
    {
        return cobblestone_world_chunk_pin_count(
            $handle,
            $position->x,
            $position->z,
        );
    }

    public static function dirty(int $handle, ChunkPos $position): bool
    {
        return cobblestone_world_chunk_dirty(
            $handle,
            $position->x,
            $position->z,
        );
    }

    public static function markPersisted(
        int $handle,
        ChunkPos $position,
        int $terrainRevision,
        int $lightRevision,
        int $lifecycleFlags,
    ): void {
        cobblestone_world_mark_persisted(
            $handle,
            $position->x,
            $position->z,
            $terrainRevision,
            $lightRevision,
            $lifecycleFlags,
        );
    }
    /** @return 0|1|2|3 0=missing, 1=pinned, 2=dirty, 3=evicted */
    public static function tryEvict(int $handle, ChunkPos $position): int
    {
        return cobblestone_world_try_evict_chunk(
            $handle,
            $position->x,
            $position->z,
        );
    }
}
