<?php

declare(strict_types=1);

namespace Cobblestone\World\Mutation;

use Cobblestone\World\BiomeId;
use Cobblestone\World\BlockState;
use Cobblestone\World\Chunk;

/** @internal */
final class PreparedChunkPatch
{
    /**
     * @param array<int, BlockState> $blocks
     * @param array<int, BiomeId> $biomes
     * @param array<int, int> $extraData
     * @param array<int, int> $skyLight
     * @param array<int, int> $blockLight
     */
    public function __construct(
        private readonly Chunk $chunk,
        private readonly int $baseRevision,
        private readonly int $revision,
        private readonly int $baseLightRevision,
        private readonly int $lightRevision,
        private readonly array $blocks,
        private readonly array $biomes,
        private readonly array $extraData,
        private readonly array $skyLight,
        private readonly array $blockLight,
    ) {
    }

    public function chunk(): Chunk
    {
        return $this->chunk;
    }

    public function changed(): bool
    {
        return $this->terrainChanged() || $this->lightChanged();
    }

    public function terrainChanged(): bool
    {
        return $this->revision !== $this->baseRevision;
    }

    public function lightChanged(): bool
    {
        return $this->lightRevision !== $this->baseLightRevision;
    }

    public function validate(): void
    {
        if ($this->chunk->revision() !== $this->baseRevision) {
            throw new MutationConflict('chunk terrain revision changed before mutation commit');
        }
        if ($this->chunk->lightRevision()->value !== $this->baseLightRevision) {
            throw new MutationConflict('chunk light revision changed before mutation commit');
        }
    }

    public function commit(): void
    {
        $this->validate();
        if (!$this->changed()) {
            return;
        }

        $blocks = $this->blocks;
        $biomes = $this->biomes;
        $extraData = $this->extraData;
        $skyLight = $this->skyLight;
        $blockLight = $this->blockLight;
        ksort($blocks);
        ksort($biomes);
        ksort($extraData);
        ksort($skyLight);
        ksort($blockLight);

        foreach ($blocks as $key => $state) {
            [$x, $y, $z] = ChunkPatch::decodeBlockKey($key);
            $this->chunk->setBlock($x, $y, $z, $state);
        }
        foreach ($biomes as $key => $biome) {
            [$x, $z] = ChunkPatch::decodeColumnKey($key);
            $this->chunk->setBiome($x, $z, $biome);
        }
        foreach ($extraData as $key => $data) {
            [$x, $y, $z] = ChunkPatch::decodeBlockKey($key);
            $this->chunk->setBlockExtraData($x, $y, $z, $data);
        }
        foreach ($skyLight as $key => $level) {
            [$x, $y, $z] = ChunkPatch::decodeBlockKey($key);
            $this->chunk->setSkyLight($x, $y, $z, $level);
        }
        foreach ($blockLight as $key => $level) {
            [$x, $y, $z] = ChunkPatch::decodeBlockKey($key);
            $this->chunk->setBlockLight($x, $y, $z, $level);
        }

        if ($this->terrainChanged()) {
            $this->chunk->commitRevision($this->baseRevision, $this->revision);
        }
        if ($this->lightChanged()) {
            $this->chunk->commitLightRevision($this->baseLightRevision, $this->lightRevision);
        }
    }
}