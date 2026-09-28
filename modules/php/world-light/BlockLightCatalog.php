<?php

declare(strict_types=1);

namespace Cobblestone\World\Light;

use Cobblestone\World\BlockStateId;

/**
 * Fixed-target MCPE 0.15.10 light metadata.
 *
 * The source oracle uses dense semantic block ordinals. This table maps those exact recovered
 * properties onto protocol-era legacy block IDs using the pinned matching BlockIds vocabulary.
 */
final class BlockLightCatalog
{
    public function propertiesForStateId(int $stateId): ?BlockLightProperties
    {
        $id = BlockStateId::blockId($stateId);
        if (!BlockLightMetadata::supports($id)) {
            return null;
        }

        return new BlockLightProperties(
            BlockLightMetadata::opacity($id),
            BlockLightMetadata::emission($id),
        );
    }

}
