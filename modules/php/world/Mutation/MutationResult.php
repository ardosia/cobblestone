<?php

declare(strict_types=1);

namespace Cobblestone\World\Mutation;

use Cobblestone\World\ChunkPos;

final readonly class MutationResult
{
    /**
     * @param list<ChunkPos> $changedChunks
     */
    public function __construct(
        public mixed $value,
        public array $changedChunks,
        public int $attempts,
    ) {
    }

    public function changed(): bool
    {
        return $this->changedChunks !== [];
    }
}
