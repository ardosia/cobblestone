<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Immutable semantic projection of one fixed-target 16x16x16 chunk section.
 *
 * The byte planes retain Cobblestone's semantic Y/Z/X section order. They are not protocol bytes.
 *
 * @internal
 */
final readonly class ChunkSectionSnapshot
{
    public function __construct(
        public string $blockIds,
        public string $blockData,
        public string $skyLight,
        public string $blockLight,
    ) {
        if (strlen($blockIds) !== ChunkSection::VOLUME) {
            throw new \ValueError('chunk section block-id snapshot has invalid length');
        }

        $nibbleBytes = intdiv(ChunkSection::VOLUME, 2);
        foreach ([
            'block data' => $blockData,
            'sky light' => $skyLight,
            'block light' => $blockLight,
        ] as $plane => $bytes) {
            if (strlen($bytes) !== $nibbleBytes) {
                throw new \ValueError("chunk section {$plane} snapshot has invalid length");
            }
        }
    }
}
