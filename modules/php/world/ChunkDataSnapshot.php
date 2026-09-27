<?php

declare(strict_types=1);

namespace Cobblestone\World;

/** Immutable terrain/biome view extracted from a ChunkSnapshot. */
final readonly class ChunkDataSnapshot
{
    public function __construct(
        private string $blockIds,
        private string $blockData,
        private string $biomes,
    ) {
        if (strlen($blockIds) !== ChunkSnapshot::BLOCK_COUNT) {
            throw new \ValueError('chunk terrain snapshot block-id length mismatch');
        }
        if (strlen($blockData) !== ChunkSnapshot::NIBBLE_BYTES) {
            throw new \ValueError('chunk terrain snapshot block-data length mismatch');
        }
        if (strlen($biomes) !== ChunkSnapshot::COLUMN_COUNT) {
            throw new \ValueError('chunk terrain snapshot biome length mismatch');
        }
    }

    public function block(int $x, int $y, int $z): ?BlockState
    {
        if (
            !WorldBounds::containsLocal($x)
            || !WorldBounds::containsLocal($z)
            || !WorldBounds::containsY($y)
        ) {
            return null;
        }

        $index = ($y << 8) | ($z << 4) | $x;
        $byte = ord($this->blockData[$index >> 1]);
        $data = ($index & 1) === 0 ? $byte & 0x0f : ($byte >> 4) & 0x0f;

        return new BlockState(ord($this->blockIds[$index]), $data);
    }

    public function biome(int $x, int $z): ?BiomeId
    {
        if (!WorldBounds::containsLocal($x) || !WorldBounds::containsLocal($z)) {
            return null;
        }

        return new BiomeId(ord($this->biomes[($z << 4) | $x]));
    }
}
