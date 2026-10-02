<?php

declare(strict_types=1);

namespace Cobblestone\World;

final readonly class TerrainEditResult
{
    public function __construct(
        public bool $changed,
        public ChunkRevision $revision,
    ) {
    }
}
