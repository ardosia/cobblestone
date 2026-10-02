<?php

declare(strict_types=1);

namespace Cobblestone\World\Region;

use Cobblestone\World\BlockPos;
use Cobblestone\World\ChunkPos;
use InvalidArgumentException;

/**
 * Deterministic execution-region mapping.
 *
 * Regions are Cobblestone ownership/scheduling territories, not Minecraft storage-region files.
 * The size is a mechanism tuning value and is not a fixed-target gameplay rule.
 */
final readonly class RegionMap implements RegionMapInterface
{
    public const DEFAULT_CHUNKS_PER_REGION = 8;

    public function __construct(
        public int $chunksPerRegion = self::DEFAULT_CHUNKS_PER_REGION,
    ) {
        if ($chunksPerRegion <= 0) {
            throw new InvalidArgumentException('chunks per region must be positive');
        }
    }

    public function forChunk(ChunkPos $chunk): RegionId
    {
        return new RegionId(
            self::floorDiv($chunk->x, $this->chunksPerRegion),
            self::floorDiv($chunk->z, $this->chunksPerRegion),
        );
    }

    public function forBlock(BlockPos $block): RegionId
    {
        return $this->forChunk($block->chunk());
    }

    private static function floorDiv(int $value, int $divisor): int
    {
        $quotient = intdiv($value, $divisor);
        if ($value < 0 && $value % $divisor !== 0) {
            --$quotient;
        }

        return $quotient;
    }
}
