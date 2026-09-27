<?php

declare(strict_types=1);

namespace Cobblestone\World;

interface BlockSource
{
    /** Scalar fixed-target state token used by performance-sensitive code. */
    public function blockStateId(BlockPos $position): int;

    /** Scalar fixed-target state token used by performance-sensitive code. */
    public function setBlockStateId(BlockPos $position, int $stateId): int;

    /** Ergonomic compatibility wrapper around blockStateId(). */
    public function block(BlockPos $position): BlockState;

    /** Ergonomic compatibility wrapper around setBlockStateId(). */
    public function setBlock(BlockPos $position, BlockState $state): BlockState;

    public function biomeAt(int $x, int $z): BiomeId;

    public function skyLight(BlockPos $position): int;

    public function blockLight(BlockPos $position): int;
}
