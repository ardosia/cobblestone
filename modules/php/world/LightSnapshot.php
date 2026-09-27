<?php

declare(strict_types=1);

namespace Cobblestone\World;

/** Immutable owned light state detached from later chunk mutations. */
final readonly class LightSnapshot
{
    public const NIBBLE_BYTES = (
        WorldBounds::CHUNK_EDGE
        * WorldBounds::CHUNK_EDGE
        * WorldBounds::WORLD_HEIGHT
    ) / 2;

    public function __construct(
        public LightRevision $revision,
        public string $sky,
        public string $block,
    ) {
        if (strlen($sky) !== self::NIBBLE_BYTES || strlen($block) !== self::NIBBLE_BYTES) {
            throw new \ValueError('chunk light snapshot has invalid plane length');
        }
    }

    public function sky(int $x, int $y, int $z): ?LightLevel
    {
        return $this->level($this->sky, $x, $y, $z);
    }

    public function block(int $x, int $y, int $z): ?LightLevel
    {
        return $this->level($this->block, $x, $y, $z);
    }

    private function level(string $plane, int $x, int $y, int $z): ?LightLevel
    {
        if (
            !WorldBounds::containsLocal($x)
            || !WorldBounds::containsLocal($z)
            || !WorldBounds::containsY($y)
        ) {
            return null;
        }

        $index = ($y << 8) | ($z << 4) | $x;
        $byte = ord($plane[$index >> 1]);
        $value = ($index & 1) === 0 ? $byte & 0x0f : ($byte >> 4) & 0x0f;

        return new LightLevel($value);
    }
}
