<?php

declare(strict_types=1);

namespace Cobblestone\World;

/** Fully preflighted chunk-local terrain replacement. */
final readonly class PreparedTerrainPatch
{
    /**
     * @param array<int, int> $blocks scalar BlockStateId tokens
     * @param array<int, BiomeId> $biomes
     */
    public function __construct(
        private int $baseRevision,
        public ChunkRevision $revision,
        public bool $changed,
        private array $blocks,
        private array $biomes,
    ) {
    }

    /** @internal */
    public function baseRevision(): int
    {
        return $this->baseRevision;
    }

    /** @internal @return array<int, int> */
    public function blocks(): array
    {
        return $this->blocks;
    }

    /** @internal @return array<int, BiomeId> */
    public function biomes(): array
    {
        return $this->biomes;
    }
}
