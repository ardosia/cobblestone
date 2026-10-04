<?php

declare(strict_types=1);

namespace Cobblestone\World\Light;

use Cobblestone\World\BlockCatalog;
use Cobblestone\World\BlockStateId;

/**
 * Fixed-target MCPE 0.15.10 light metadata projected from the authoritative block catalog.
 */
final class BlockLightCatalog
{
    public function propertiesForStateId(int $stateId): ?BlockLightProperties
    {
        if ($stateId < 0 || $stateId > BlockStateId::MAX) {
            return null;
        }

        $id = $stateId >> 4;
        if (!BlockCatalog::supports($id)) {
            return null;
        }

        return new BlockLightProperties(
            BlockCatalog::lightBlock($id),
            BlockCatalog::lightEmission($id),
        );
    }
}
