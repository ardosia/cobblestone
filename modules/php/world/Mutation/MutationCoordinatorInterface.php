<?php

declare(strict_types=1);

namespace Cobblestone\World\Mutation;

use Closure;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\WorldEdit;

/** @internal Replay/commit contract consumed by the World aggregate. */
interface MutationCoordinatorInterface
{
    /**
     * @param Closure(WorldEdit): mixed $operation
     * @param list<ChunkPos> $hints
     */
    public function run(Closure $operation, array $hints = []): MutationResult;
}
