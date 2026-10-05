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

    /** @return array{x: int, z: int} */
    public static function overworldSpawn(int $seed): array
    {
        if (!\function_exists('cobblestone_world_overworld_spawn')) {
            throw new RuntimeException(
                'native Overworld spawn search is unavailable; build/load cobblestone_core_php',
            );
        }

        $payload = cobblestone_world_overworld_spawn($seed);
        if (strlen($payload) !== 8) {
            throw new \UnexpectedValueException('native Overworld spawn projection width mismatch');
        }
        $decoded = unpack('Vx/Vz', $payload);
        if (!is_array($decoded) || !isset($decoded['x'], $decoded['z'])) {
            throw new \UnexpectedValueException('native Overworld spawn projection decode failed');
        }

        return [
            'x' => self::signed32($decoded['x']),
            'z' => self::signed32($decoded['z']),
        ];
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

    private static function signed32(int $value): int
    {
        return $value >= 0x80000000 ? $value - 0x100000000 : $value;
    }
}
