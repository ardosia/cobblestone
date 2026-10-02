<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * World-facing capability required by the fixed-target propagation substrate.
 *
 * Hot-path light access is scalar-first: state ids and light levels cross this seam as ints.
 * Implementations decide residency/staging. A null block/light/sky answer means unavailable.
 */
interface LightAccess
{
    public function neighborhoodAvailable(BlockPos $position): bool;

    public function blockStateId(BlockPos $position): ?int;

    public function storedLight(LightLayer $layer, BlockPos $position): ?int;

    public function setStoredLight(
        LightLayer $layer,
        BlockPos $position,
        int $level,
    ): bool;

    public function canSeeSky(BlockPos $position): ?bool;
}
