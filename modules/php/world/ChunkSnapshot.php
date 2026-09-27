<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Immutable semantic snapshot used at bulk native/wire boundaries.
 *
 * The planes are gameplay/world state, not a protocol packet. Block and nibble planes use
 * global Y/Z/X order so protocol-specific transposition remains native codec machinery.
 */
final readonly class ChunkSnapshot
{
    public const BLOCK_COUNT = WorldBounds::CHUNK_EDGE * WorldBounds::CHUNK_EDGE * WorldBounds::WORLD_HEIGHT;
    public const NIBBLE_BYTES = self::BLOCK_COUNT / 2;
    public const COLUMN_COUNT = WorldBounds::CHUNK_EDGE * WorldBounds::CHUNK_EDGE;

    /**
     * @param array<int, int> $extraData sparse semantic block-extra-data map
     */
    public function __construct(
        public ChunkPos $position,
        public int $revision,
        public string $blockIds,
        public string $blockData,
        public string $skyLight,
        public string $blockLight,
        public string $biomes,
        public string $heightMap,
        public array $extraData,
    ) {
        if (strlen($blockIds) !== self::BLOCK_COUNT) {
            throw new \ValueError('chunk block-id snapshot has invalid length');
        }

        foreach ([
            'block data' => $blockData,
            'sky light' => $skyLight,
            'block light' => $blockLight,
        ] as $plane => $bytes) {
            if (strlen($bytes) !== self::NIBBLE_BYTES) {
                throw new \ValueError("chunk {$plane} snapshot has invalid length");
            }
        }

        if (strlen($biomes) !== self::COLUMN_COUNT || strlen($heightMap) !== self::COLUMN_COUNT) {
            throw new \ValueError('chunk column snapshot has invalid length');
        }

        foreach ($extraData as $key => $value) {
            if (!is_int($key) || $key < 0 || $key > 0xffff) {
                throw new \ValueError('chunk extra-data key must fit 16 bits');
            }
            if (!is_int($value) || $value < 0 || $value > 0xffff) {
                throw new \ValueError('chunk extra-data value must fit 16 bits');
            }
        }
    }
}
