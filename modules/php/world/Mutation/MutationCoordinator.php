<?php

declare(strict_types=1);

namespace Cobblestone\World\Mutation;

use Closure;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\Region\RegionMap;
use Cobblestone\World\World;
use LogicException;

/**
 * @internal
 *
 * Owner-runtime mutation coordinator.
 *
 * The current implementation commits on one owning PHP runtime. Discovery/replay and revision
 * gates are already explicit so native region routing can later acquire the complete region set
 * before the final attempt without changing the World::mutate() API.
 */
final class MutationCoordinator
{
    private bool $active = false;

    public function __construct(
        private readonly World $world,
        private readonly RegionMap $regions,
        private readonly int $maxAttempts = 8,
    ) {
        if ($maxAttempts <= 0) {
            throw new \InvalidArgumentException('mutation max attempts must be positive');
        }
    }

    /**
     * @param Closure(WorldMutation): mixed $operation
     * @param list<ChunkPos> $hints
     */
    public function run(Closure $operation, array $hints = []): MutationResult
    {
        if ($this->active) {
            throw new LogicException('nested World::mutate() calls are not allowed; use the active WorldMutation');
        }

        $allowedChunks = [];
        foreach ($hints as $hint) {
            $allowedChunks[$hint->key()] = true;
        }

        $this->active = true;
        try {
            for ($attempt = 1; $attempt <= $this->maxAttempts; ++$attempt) {
                $mutation = new WorldMutation($this->world);
                $value = $operation($mutation);
                $patches = $this->orderPatches($mutation->patches());

                $discovered = array_diff_key($patches, $allowedChunks);
                if ($discovered !== []) {
                    foreach ($patches as $key => $_patch) {
                        $allowedChunks[$key] = true;
                    }
                    continue;
                }

                try {
                    $prepared = [];
                    foreach ($patches as $patch) {
                        $prepared[] = $patch->prepare();
                    }
                    foreach ($prepared as $patch) {
                        $patch->validate();
                    }
                } catch (MutationConflict) {
                    continue;
                }

                $changedChunks = [];
                foreach ($prepared as $patch) {
                    if (!$patch->changed()) {
                        continue;
                    }
                    $patch->commit();
                    $changedChunks[] = $patch->chunk()->position();
                }

                return new MutationResult(
                    $value,
                    $changedChunks,
                    $attempt,
                );
            }
        } finally {
            $this->active = false;
        }

        throw new MutationConflict(
            "world mutation did not stabilize within {$this->maxAttempts} attempts",
        );
    }

    /**
     * @param array<string, ChunkPatch> $patches
     * @return array<string, ChunkPatch>
     */
    private function orderPatches(array $patches): array
    {
        uasort(
            $patches,
            function (ChunkPatch $left, ChunkPatch $right): int {
                $leftPosition = $left->chunk()->position();
                $rightPosition = $right->chunk()->position();
                $leftRegion = $this->regions->forChunk($leftPosition);
                $rightRegion = $this->regions->forChunk($rightPosition);

                return $leftRegion->x <=> $rightRegion->x
                    ?: $leftRegion->z <=> $rightRegion->z
                    ?: $leftPosition->x <=> $rightPosition->x
                    ?: $leftPosition->z <=> $rightPosition->z;
            },
        );

        return $patches;
    }
}
