<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Shared packed coordinate keys for staged chunk-local mutation data.
 *
 * @internal
 */
final class ChunkCoordinateKey
{
    private function __construct()
    {
    }

    public static function block(int $x, int $y, int $z): int
    {
        return ($y << 8) | ($z << 4) | $x;
    }

    public static function blockOrNull(int $x, int $y, int $z): ?int
    {
        if (
            !WorldBounds::containsLocal($x)
            || !WorldBounds::containsLocal($z)
            || !WorldBounds::containsY($y)
        ) {
            return null;
        }

        return self::block($x, $y, $z);
    }

    /** @return array{int, int, int} */
    public static function decodeBlock(int $key): array
    {
        return [$key & 0x0f, ($key >> 8) & 0x7f, ($key >> 4) & 0x0f];
    }

    public static function column(int $x, int $z): int
    {
        return ($z << 4) | $x;
    }

    public static function columnOrNull(int $x, int $z): ?int
    {
        if (!WorldBounds::containsLocal($x) || !WorldBounds::containsLocal($z)) {
            return null;
        }

        return self::column($x, $z);
    }

    /** @return array{int, int} */
    public static function decodeColumn(int $key): array
    {
        return [$key & 0x0f, ($key >> 4) & 0x0f];
    }
}
