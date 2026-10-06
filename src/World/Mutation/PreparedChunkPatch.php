<?php

declare(strict_types=1);

namespace Cobblestone\World\Mutation;

use Cobblestone\World\BiomeColumn;
use Cobblestone\World\Chunk;

/** @internal */
final class PreparedChunkPatch
{
    /**
     * @param array<int, int> $blocks scalar BlockStateId tokens
     * @param array<int, BiomeColumn> $biomes
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
    ) {}

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

        $this->chunk->applyPatch(
            $this->baseRevision,
            $this->revision,
            $this->baseLightRevision,
            $this->lightRevision,
            $blocks,
            $biomes,
            $extraData,
            $skyLight,
            $blockLight,
        );
    }
}
