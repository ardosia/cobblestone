<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Reads the private native immutable chunk snapshot projection.
 *
 * @internal
 */
final class NativeChunkSnapshotProjection
{
    private function __construct()
    {
    }

    public static function read(int $handle, ChunkPos $position): string
    {
        return cobblestone_world_snapshot(
            $handle,
            $position->x,
            $position->z,
        );
    }
}
