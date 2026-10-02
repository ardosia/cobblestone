<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Replayable semantic edit surface used by World::edit().
 *
 * The callback may be replayed before commit when the edit discovers additional chunks, so side
 * effects must stay inside the edit itself. Scalar state IDs remain the hot-path currency while
 * BlockState methods provide the ergonomic surface.
 */
interface WorldEdit
{
    public function blockStateId(BlockPos $position): int;

    public function setBlockStateId(BlockPos $position, int $stateId): int;

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
