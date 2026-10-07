<?php

declare(strict_types=1);

namespace Cobblestone\Native\Session;

use Cobblestone\World\ChunkPos;

/** @internal */
final readonly class ChunkWork
{
    /** @param list<ChunkPos> $positions */
    public function __construct(
        public int $sessionId,
        public ChunkWorkKind $kind,
        public array $positions,
    ) {}
}
