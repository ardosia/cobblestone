<?php

declare(strict_types=1);

namespace Cobblestone\World;

final class ChunkLoadPending extends \RuntimeException
{
    public function __construct(
        public readonly ChunkPos $position,
        public readonly NativeChunkLoadStatus $status,
    ) {
        parent::__construct(
            "chunk {$position->x}:{$position->z} is still loading from durable storage",
        );
    }
}
