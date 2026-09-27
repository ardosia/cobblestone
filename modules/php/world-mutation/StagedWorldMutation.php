<?php

declare(strict_types=1);

namespace Cobblestone\World\Mutation;

use Cobblestone\World\BiomeId;
use Cobblestone\World\BlockPos;
use Cobblestone\World\BlockState;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\ChunkSource;
use ValueError;

/** @internal Owner-runtime staged implementation of the public mutation surface. */
final class StagedWorldMutation implements WorldMutation
{
    /** @var array<string, ChunkPatch> */
    private array $patches = [];

    public function __construct(
        private readonly ChunkSource $chunks,
    ) {
    }

    public function block(BlockPos $position): BlockState
    {
        $patch = $this->patch($position);

        return $patch->block($position->localX(), $position->y, $position->localZ());
    }

    public function setBlock(BlockPos $position, BlockState $state): BlockState
    {
        $patch = $this->patch($position);

        return $patch->setBlock($position->localX(), $position->y, $position->localZ(), $state);
    }

    public function biomeAt(int $x, int $z): BiomeId
    {
        $chunk = ChunkPos::fromBlock($x, $z);
        $patch = $this->patchForChunk($chunk);

        return $patch->biome(ChunkPos::localCoordinate($x), ChunkPos::localCoordinate($z));
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
