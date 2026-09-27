<?php

declare(strict_types=1);

namespace Cobblestone\World\Mutation;

use Cobblestone\World\ChunkPos;
use Cobblestone\World\Region\RegionId;

final readonly class MutationResult
{
    /**
     * @param list<ChunkPos> $changedChunks
     * @param list<RegionId> $regions
     */
    public function __construct(
        public mixed $value,
        public array $changedChunks,
        public array $regions,
        public int $attempts,
    ) {
    }

    public function changed(): bool
    {
        return $this->changedChunks !== [];
    }
}
