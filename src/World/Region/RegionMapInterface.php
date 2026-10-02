<?php

declare(strict_types=1);

namespace Cobblestone\World\Region;

use Cobblestone\World\BlockPos;
use Cobblestone\World\ChunkPos;

/** @internal Execution-region routing contract owned by the base world model. */
interface RegionMapInterface
{
    public function forChunk(ChunkPos $chunk): RegionId;

    public function forBlock(BlockPos $block): RegionId;
}
