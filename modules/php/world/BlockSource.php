<?php

declare(strict_types=1);

namespace Cobblestone\World;

interface BlockSource
{
    public function block(BlockPos $position): BlockState;

    public function setBlock(BlockPos $position, BlockState $state): BlockState;

    public function biomeAt(int $x, int $z): BiomeId;

    public function skyLight(BlockPos $position): int;

    public function blockLight(BlockPos $position): int;
}
