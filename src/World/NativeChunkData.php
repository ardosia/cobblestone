<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Native scalar terrain, biome, light, height-map, and extra-data operations for one chunk.
 *
 * @internal
 */
final class NativeChunkData
{
    private function __construct()
    {
    }

    public static function blockStateId(int $handle, ChunkPos $position, int $x, int $y, int $z): int
    {
        return cobblestone_world_block_state(
            $handle,
            $position->x,
            $position->z,
            $x,
            $y,
            $z,
        );
    }

    public static function setBlockStateId(
        int $handle,
        ChunkPos $position,
        int $x,
        int $y,
        int $z,
        int $stateId,
    ): int {
        return cobblestone_world_set_block_state(
            $handle,
            $position->x,
            $position->z,
            $x,
            $y,
            $z,
            $stateId,
        );
    }
    public static function fillLayers(
        int $handle,
        ChunkPos $position,
        int $startY,
        int $count,
        int $stateId,
    ): void {
        cobblestone_world_fill_layers(
            $handle,
            $position->x,
            $position->z,
            $startY,
            $count,
            $stateId,
        );
    }

    public static function biome(int $handle, ChunkPos $position, int $x, int $z): int
    {
        return cobblestone_world_biome(
            $handle,
            $position->x,
            $position->z,
            $x,
            $z,
        );
    }

    public static function setBiome(int $handle, ChunkPos $position, int $x, int $z, int $biome): int
    {
        return cobblestone_world_set_biome(
            $handle,
            $position->x,
            $position->z,
            $x,
            $z,
            $biome,
        );
    }

    public static function skyLight(int $handle, ChunkPos $position, int $x, int $y, int $z): int
    {
        return cobblestone_world_sky_light(
            $handle,
            $position->x,
            $position->z,
            $x,
            $y,
            $z,
        );
    }

    public static function setSkyLight(
        int $handle,
        ChunkPos $position,
        int $x,
        int $y,
        int $z,
        int $level,
    ): int {
        return cobblestone_world_set_sky_light(
            $handle,
            $position->x,
            $position->z,
            $x,
            $y,
            $z,
            $level,
        );
    }

    public static function fillSkyLightFrom(int $handle, ChunkPos $position, int $y, int $level): void
    {
        cobblestone_world_fill_sky_light_from(
            $handle,
            $position->x,
            $position->z,
            $y,
            $level,
        );
    }

    public static function blockLight(int $handle, ChunkPos $position, int $x, int $y, int $z): int
    {
        return cobblestone_world_block_light(
            $handle,
            $position->x,
            $position->z,
            $x,
            $y,
            $z,
        );
    }
    public static function setBlockLight(
        int $handle,
        ChunkPos $position,
        int $x,
        int $y,
        int $z,
        int $level,
    ): int {
        return cobblestone_world_set_block_light(
            $handle,
            $position->x,
            $position->z,
            $x,
            $y,
            $z,
            $level,
        );
    }

    public static function heightMap(int $handle, ChunkPos $position, int $x, int $z): int
    {
        return cobblestone_world_height_map(
            $handle,
            $position->x,
            $position->z,
            $x,
            $z,
        );
    }

    public static function recalculateHeightMap(int $handle, ChunkPos $position): void
    {
        cobblestone_world_recalculate_height_map(
            $handle,
            $position->x,
            $position->z,
        );
    }

    public static function blockExtraData(int $handle, ChunkPos $position, int $x, int $y, int $z): int
    {
        return cobblestone_world_block_extra_data(
            $handle,
            $position->x,
            $position->z,
            $x,
            $y,
            $z,
        );
    }
    public static function setBlockExtraData(
        int $handle,
        ChunkPos $position,
        int $x,
        int $y,
        int $z,
        int $data,
    ): int {
        return cobblestone_world_set_block_extra_data(
            $handle,
            $position->x,
            $position->z,
            $x,
            $y,
            $z,
            $data,
        );
    }
}
