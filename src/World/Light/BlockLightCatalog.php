<?php

declare(strict_types=1);

namespace Cobblestone\World\Light;

use Cobblestone\World\BlockStateId;
use Cobblestone\World\BlockType;

/**
 * Scalar-state adapter for fixed-target block light metadata.
 *
 * Semantic metadata lives on BlockType; this class exists only for the scalar propagation boundary.
 */
final class BlockLightCatalog
{
    public function propertiesForStateId(int $stateId): ?BlockLightProperties
    {
        if ($stateId < 0 || $stateId > BlockStateId::MAX) {
            return null;
        }

        return BlockType::tryFrom($stateId >> 4)?->lightProperties();
    }
}
