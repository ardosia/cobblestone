<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * World-facing capability required by the fixed-target propagation substrate.
 *
 * Implementations decide residency/staging. A null block/light/sky answer means unavailable.
 */
interface LightAccess
{
    public function neighborhoodAvailable(BlockPos $position): bool;

    public function blockState(BlockPos $position): ?BlockState;

    public function storedLight(LightLayer $layer, BlockPos $position): ?LightLevel;

    public function setStoredLight(
        LightLayer $layer,
        BlockPos $position,
        LightLevel $level,
    ): bool;

    public function canSeeSky(BlockPos $position): ?bool;
}
