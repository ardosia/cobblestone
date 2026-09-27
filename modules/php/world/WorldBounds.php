<?php

declare(strict_types=1);

namespace Cobblestone\World;

final class WorldBounds
{
    public const CHUNK_EDGE = 16;
    public const SECTION_EDGE = 16;
    public const SECTION_COUNT = 8;
    public const WORLD_HEIGHT = self::SECTION_EDGE * self::SECTION_COUNT;
    public const MIN_Y = 0;
    public const MAX_Y = self::WORLD_HEIGHT - 1;

    private function __construct()
    {
    }

    public static function containsY(int $y): bool
    {
        return $y >= self::MIN_Y && $y <= self::MAX_Y;
    }

    public static function containsLocal(int $coordinate): bool
    {
        return $coordinate >= 0 && $coordinate < self::CHUNK_EDGE;
    }
}
