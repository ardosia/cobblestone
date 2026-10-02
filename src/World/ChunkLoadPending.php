<?php

declare(strict_types=1);

namespace Cobblestone\World;

use Cobblestone\Native\World\LoadStatus;

final class ChunkLoadPending extends \RuntimeException
{
    public function __construct(
        public readonly ChunkPos $position,
        public readonly LoadStatus $status,
    ) {
        parent::__construct(
            "chunk {$position->x}:{$position->z} is still loading from durable storage",
        );
    }
}
