<?php

declare(strict_types=1);

namespace Cobblestone\World\Light;

use Closure;
use Cobblestone\World\BlockPos;
use Cobblestone\World\LightAccess;
use Cobblestone\World\LightLayer;
use Cobblestone\World\LightLevel;
use Cobblestone\World\LightUpdate;
use Cobblestone\World\WorldBounds;

/**
 * Fixed-target low-level light algorithm.
 *
 * Scheduling/coalescing/budgets are caller-owned. Neighbor submission order matches the pinned
 * Ardosia implementation recovered from MCPE/Win10 0.15.10 LightUpdate::updateFast.
 */
final readonly class LightPropagator
{
    public function __construct(private BlockLightCatalog $catalog)
    {
    }

    /** @param Closure(LightUpdate): void $submit */
    public function apply(LightAccess $access, LightUpdate $update, Closure $submit): void
    {
        $minY = max($update->min->y, WorldBounds::MIN_Y);
        $maxY = min($update->max->y, WorldBounds::MAX_Y);
        if ($minY > $maxY) {
            return;
        }

        for ($x = $update->min->x; $x <= $update->max->x; ++$x) {
            for ($z = $update->min->z; $z <= $update->max->z; ++$z) {
                if (!$access->neighborhoodAvailable(new BlockPos($x, $update->min->y, $z))) {
                    continue;
                }

                for ($y = $minY; $y <= $maxY; ++$y) {
                    $this->applyCell(
                        $access,
                        $update,
                        $maxY,
                        new BlockPos($x, $y, $z),
                        $submit,
                    );
                }
            }
        }
    }

    /** @param Closure(LightUpdate): void $submit */
    private function applyCell(
        LightAccess $access,
        LightUpdate $update,
        int $maxY,
        BlockPos $position,
        Closure $submit,
    ): void {
        $stateId = $access->blockStateId($position);
        if ($stateId === null) {
            throw new LightPropagationException('unsupported or unavailable block state');
        }

        $properties = $this->catalog->propertiesForStateId($stateId);
        if ($properties === null) {
            throw new LightPropagationException('unsupported fixed-target block state');
        }

        $old = $access->storedLight($update->layer, $position);
        if ($old === null) {
            throw new LightPropagationException('stored light is unavailable');
        }

        $source = $this->localSource(
            $access,
            $update->layer,
            $position,
            $properties->lightEmission,
        );
        $attenuation = $properties->propagationAttenuation();
        $propagated = $attenuation >= LightLevel::MAX_VALUE && $source === 0
            ? 0
            : max(0, $this->maxNeighborLight($access, $update->layer, $position) - $attenuation);
        $next = max($source, $propagated);
        if ($next === $old->value) {
            return;
        }

        $nextLevel = new LightLevel($next);
        if (!$access->setStoredLight($update->layer, $position, $nextLevel)) {
            throw new LightPropagationException('stored light became unavailable');
        }

        $inherited = max(0, $next - 1);
        foreach ($this->propagationNeighbors($position, $update->max, $maxY) as $neighbor) {
            if ($neighbor === null || !WorldBounds::containsY($neighbor->y)) {
                continue;
            }

            $neighborStateId = $access->blockStateId($neighbor);
            if ($neighborStateId === null) {
                throw new LightPropagationException('neighbor block state is unavailable');
            }
            $neighborProperties = $this->catalog->propertiesForStateId($neighborStateId);
            if ($neighborProperties === null) {
                throw new LightPropagationException('unsupported neighbor block state');
            }

            $expected = max(
                $inherited,
                $this->localSource(
                    $access,
                    $update->layer,
                    $neighbor,
                    $neighborProperties->lightEmission,
                ),
            );
            $actual = $access->storedLight($update->layer, $neighbor);
            if ($actual === null) {
                throw new LightPropagationException('neighbor stored light is unavailable');
            }

            if ($actual->value !== $expected) {
                $submit(LightUpdate::point($update->layer, $neighbor));
            }
        }
    }

    private function localSource(
        LightAccess $access,
        LightLayer $layer,
        BlockPos $position,
        int $blockEmission,
    ): int {
        if ($layer === LightLayer::Block) {
            return $blockEmission;
        }

        $exposed = $access->canSeeSky($position);
        if ($exposed === null) {
            throw new LightPropagationException('sky exposure is unavailable');
        }

        return $exposed ? LightLevel::MAX_VALUE : LightLevel::MIN_VALUE;
    }

    private function maxNeighborLight(
        LightAccess $access,
        LightLayer $layer,
        BlockPos $position,
    ): int {
        $maximum = 0;
        foreach ($this->sixNeighbors($position) as $neighbor) {
            if (!WorldBounds::containsY($neighbor->y)) {
                continue;
            }

            $level = $access->storedLight($layer, $neighbor);
            if ($level === null) {
                throw new LightPropagationException('neighbor stored light is unavailable');
            }
            $maximum = max($maximum, $level->value);
        }

        return $maximum;
    }

    /** @return array{?BlockPos, ?BlockPos, ?BlockPos, ?BlockPos, ?BlockPos, ?BlockPos} */
    private function propagationNeighbors(
        BlockPos $position,
        BlockPos $updateMax,
        int $maxY,
    ): array {
        $positiveX = $position->x + 1;
        $positiveY = $position->y + 1;
        $positiveZ = $position->z + 1;

        return [
            new BlockPos($position->x - 1, $position->y, $position->z),
            new BlockPos($position->x, $position->y - 1, $position->z),
            new BlockPos($position->x, $position->y, $position->z - 1),
            $positiveX >= $updateMax->x
                ? new BlockPos($positiveX, $position->y, $position->z)
                : null,
            $positiveY >= $maxY
                ? new BlockPos($position->x, $positiveY, $position->z)
                : null,
            $positiveZ >= $updateMax->z
                ? new BlockPos($position->x, $position->y, $positiveZ)
                : null,
        ];
    }

    /** @return array{BlockPos, BlockPos, BlockPos, BlockPos, BlockPos, BlockPos} */
    private function sixNeighbors(BlockPos $position): array
    {
        return [
            new BlockPos($position->x - 1, $position->y, $position->z),
            new BlockPos($position->x + 1, $position->y, $position->z),
            new BlockPos($position->x, $position->y - 1, $position->z),
            new BlockPos($position->x, $position->y + 1, $position->z),
            new BlockPos($position->x, $position->y, $position->z - 1),
            new BlockPos($position->x, $position->y, $position->z + 1),
        ];
    }
}
