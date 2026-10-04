<?php

declare(strict_types=1);

namespace Cobblestone\Native\World;

use RuntimeException;

/** @internal Coarse fixed-target biome-source bridge. */
final class BiomeSource
{
    private function __construct()
    {
    }

    public static function overworld(
        int $seed,
        int $x,
        int $z,
        int $width,
        int $height,
    ): string {
        if (!\function_exists('cobblestone_world_overworld_biomes')) {
            throw new RuntimeException(
                'native Overworld biome source is unavailable; build/load cobblestone_core_php',
            );
        }

        return cobblestone_world_overworld_biomes($seed, $x, $z, $width, $height);
    }
}
