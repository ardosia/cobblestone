<?php

declare(strict_types=1);

namespace Cobblestone\World\Light;

use Cobblestone\World\ChunkPos;

final readonly class LightPropagationResult
{
    /** @param list<ChunkPos> $changedChunks */
    public function __construct(
        public int $processedUpdates,
        public array $changedChunks,
    ) {
    }

    public function changed(): bool
    {
        return $this->changedChunks !== [];
    }
}
