<?php

declare(strict_types=1);

namespace Cobblestone\World\Mutation;

use Cobblestone\World\BiomeId;
use Cobblestone\World\BlockPos;
use Cobblestone\World\BlockState;

/**
 * Replayable semantic mutation surface exposed by World::mutate().
 *
 * Implementations may replay an operation before commit, so callers must keep side effects inside
 * the mutation itself.
 */
interface WorldMutation
{
    public function block(BlockPos $position): BlockState;

    public function setBlock(BlockPos $position, BlockState $state): BlockState;

    public function biomeAt(int $x, int $z): BiomeId;

    public function setBiomeAt(int $x, int $z, BiomeId $biome): BiomeId;

    public function blockExtraData(BlockPos $position): int;

    public function setBlockExtraData(BlockPos $position, int $data): int;

    public function skyLight(BlockPos $position): int;

    public function setSkyLight(BlockPos $position, int $level): int;

    public function blockLight(BlockPos $position): int;

    public function setBlockLight(BlockPos $position, int $level): int;
}
