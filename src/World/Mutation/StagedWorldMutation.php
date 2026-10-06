<?php

declare(strict_types=1);

namespace Cobblestone\World\Mutation;

use Cobblestone\World\BiomeColumn;
use Cobblestone\World\BiomeId;
use Cobblestone\World\BlockPos;
use Cobblestone\World\BlockState;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\MainChunkSource;
use Cobblestone\World\WorldEdit;
use ValueError;

/** @internal Owner-runtime staged implementation of the public world-edit surface. */
final class StagedWorldMutation implements WorldEdit
{
    /** @var array<string, ChunkPatch> */
    private array $patches = [];

    public function __construct(
        private readonly MainChunkSource $chunks,
    ) {}

    public function blockStateId(BlockPos $position): int
    {
        $patch = $this->patch($position);

        return $patch->blockStateId($position->localX(), $position->y, $position->localZ());
    }

    public function setBlockStateId(BlockPos $position, int $stateId): int
    {
        $patch = $this->patch($position);

        return $patch->setBlockStateId(
            $position->localX(),
            $position->y,
            $position->localZ(),
            $stateId,
        );
    }

    public function block(BlockPos $position): BlockState
    {
        return BlockState::fromId($this->blockStateId($position));
    }

    public function setBlock(BlockPos $position, BlockState $state): BlockState
    {
        return BlockState::fromId($this->setBlockStateId($position, $state->stateId()));
    }

    public function biomeColumnAt(int $x, int $z): BiomeColumn
    {
        $chunk = ChunkPos::fromBlock($x, $z);
        $patch = $this->patchForChunk($chunk);

        return $patch->biomeColumn(ChunkPos::localCoordinate($x), ChunkPos::localCoordinate($z));
    }

    public function biomeAt(int $x, int $z): BiomeId
    {
        $chunk = ChunkPos::fromBlock($x, $z);
        $patch = $this->patchForChunk($chunk);

        return $patch->biome(ChunkPos::localCoordinate($x), ChunkPos::localCoordinate($z));
    }

    public function biomeColorAt(int $x, int $z): int
    {
        return $this->biomeColumnAt($x, $z)->color;
    }

    public function setBiomeColumnAt(int $x, int $z, BiomeColumn $biome): BiomeColumn
    {
        $chunk = ChunkPos::fromBlock($x, $z);
        $patch = $this->patchForChunk($chunk);

        return $patch->setBiomeColumn(
            ChunkPos::localCoordinate($x),
            ChunkPos::localCoordinate($z),
            $biome,
        );
    }

    public function setBiomeAt(int $x, int $z, BiomeId $biome): BiomeId
    {
        $chunk = ChunkPos::fromBlock($x, $z);
        $patch = $this->patchForChunk($chunk);

        return $patch->setBiome(
            ChunkPos::localCoordinate($x),
            ChunkPos::localCoordinate($z),
            $biome,
        );
    }

    public function setBiomeColorAt(int $x, int $z, int $color): int
    {
        $chunk = ChunkPos::fromBlock($x, $z);
        $patch = $this->patchForChunk($chunk);

        return $patch->setBiomeColor(
            ChunkPos::localCoordinate($x),
            ChunkPos::localCoordinate($z),
            $color,
        );
    }

    public function blockExtraData(BlockPos $position): int
    {
        $patch = $this->patch($position);

        return $patch->blockExtraData($position->localX(), $position->y, $position->localZ());
    }

    public function setBlockExtraData(BlockPos $position, int $data): int
    {
        $patch = $this->patch($position);

        return $patch->setBlockExtraData(
            $position->localX(),
            $position->y,
            $position->localZ(),
            $data,
        );
    }

    public function skyLight(BlockPos $position): int
    {
        $patch = $this->patch($position);

        return $patch->skyLight($position->localX(), $position->y, $position->localZ());
    }

    public function setSkyLight(BlockPos $position, int $level): int
    {
        $patch = $this->patch($position);

        return $patch->setSkyLight(
            $position->localX(),
            $position->y,
            $position->localZ(),
            $level,
        );
    }

    public function blockLight(BlockPos $position): int
    {
        $patch = $this->patch($position);

        return $patch->blockLight($position->localX(), $position->y, $position->localZ());
    }

    public function setBlockLight(BlockPos $position, int $level): int
    {
        $patch = $this->patch($position);

        return $patch->setBlockLight(
            $position->localX(),
            $position->y,
            $position->localZ(),
            $level,
        );
    }

    /**
     * @return array<string, ChunkPatch>
     */
    public function patches(): array
    {
        $patches = $this->patches;
        ksort($patches);

        return $patches;
    }

    private function patch(BlockPos $position): ChunkPatch
    {
        if (!$position->isInsideWorld()) {
            throw new ValueError('mutation block y must be in fixed-target range 0..127');
        }

        return $this->patchForChunk($position->chunk());
    }

    private function patchForChunk(ChunkPos $position): ChunkPatch
    {
        $key = $position->key();
        if (isset($this->patches[$key])) {
            return $this->patches[$key];
        }

        $chunk = $this->chunks->getOrGenerate($position);

        return $this->patches[$key] = new ChunkPatch($chunk);
    }
}
