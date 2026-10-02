<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Native terrain/light revision operations for one chunk.
 *
 * @internal
 */
final class NativeChunkRevision
{
    private function __construct()
    {
    }

    public static function terrain(int $handle, ChunkPos $position): int
    {
        return cobblestone_world_terrain_revision(
            $handle,
            $position->x,
            $position->z,
        );
    }

    public static function light(int $handle, ChunkPos $position): int
    {
        return cobblestone_world_light_revision(
            $handle,
            $position->x,
            $position->z,
        );
    }

    public static function commitTerrain(
        int $handle,
        ChunkPos $position,
        int $expected,
        int $next,
    ): void {
        cobblestone_world_commit_terrain_revision(
            $handle,
            $position->x,
            $position->z,
            $expected,
            $next,
        );
    }

    public static function commitLight(
        int $handle,
        ChunkPos $position,
        int $expected,
        int $next,
    ): void {
        cobblestone_world_commit_light_revision(
            $handle,
            $position->x,
            $position->z,
            $expected,
            $next,
        );
    }
}
