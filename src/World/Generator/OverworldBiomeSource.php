<?php

declare(strict_types=1);

namespace Cobblestone\World\Generator;

use Cobblestone\Native\World\BiomeSource as NativeBiomeSource;
use Cobblestone\World\BiomeArea;
use Cobblestone\World\BiomeId;
use Cobblestone\World\ChunkPos;
use ValueError;

/**
 * Exact fixed-target Overworld layered biome source.
 *
 * Terrain generation is deliberately separate; this object answers only biome identity.
 */
final readonly class OverworldBiomeSource implements BiomeSource
{
    private const MAX_AREA_EDGE = 64;

    public function __construct(
        public int $seed,
    ) {
        if ($seed < -0x80000000 || $seed > 0x7fffffff) {
            throw new ValueError('MCPE 0.15.10 Overworld biome seed must fit signed 32 bits');
        }
    }

    public function biomeAt(int $x, int $z): BiomeId
    {
        return $this->area($x, $z, 1, 1)->idAt(0, 0);
    }

    public function area(int $x, int $z, int $width, int $height): BiomeArea
    {
        self::assertCoordinate($x, 'biome x');
        self::assertCoordinate($z, 'biome z');
        self::assertEdge($width, 'biome area width');
        self::assertEdge($height, 'biome area height');
        self::assertAreaEnd($x, $width, 'biome x');
        self::assertAreaEnd($z, $height, 'biome z');

        return BiomeArea::fromBinary(
            $x,
            $z,
            $width,
            $height,
            NativeBiomeSource::overworld($this->seed, $x, $z, $width, $height),
        );
    }

    public function chunk(ChunkPos $position): BiomeArea
    {
        $x = $position->x * 16;
        $z = $position->z * 16;

        return $this->area($x, $z, 16, 16);
    }

    private static function assertCoordinate(int $value, string $field): void
    {
        if ($value < -0x80000000 || $value > 0x7fffffff) {
            throw new ValueError("{$field} must fit signed 32 bits");
        }
    }

    private static function assertEdge(int $value, string $field): void
    {
        if ($value <= 0 || $value > self::MAX_AREA_EDGE) {
            throw new ValueError("{$field} must be in range 1.." . self::MAX_AREA_EDGE);
        }
    }

    private static function assertAreaEnd(int $origin, int $length, string $field): void
    {
        if ($origin > 0x7fffffff - ($length - 1)) {
            throw new ValueError("{$field} area exceeds signed 32-bit coordinates");
        }
    }
}
