<?php

declare(strict_types=1);

namespace Cobblestone\World;

/** One fixed-target light-update region. */
final readonly class LightUpdate
{
    public function __construct(
        public LightLayer $layer,
        public BlockPos $min,
        public BlockPos $max,
    ) {}

    public static function point(LightLayer $layer, BlockPos $position): self
    {
        return new self($layer, $position, $position);
    }
}
