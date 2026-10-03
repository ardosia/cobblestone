<?php

declare(strict_types=1);

namespace Cobblestone\World;

/** Immutable terrain/biome view extracted from a ChunkSnapshot. */
final readonly class ChunkDataSnapshot
{
    public function __construct(
        private string $blockIds,
        private string $blockData,
        private string $biomeWords,
    ) {
        if (strlen($blockIds) !== ChunkSnapshot::BLOCK_COUNT) {
            throw new \ValueError('chunk terrain snapshot block-id length mismatch');
        }
        if (strlen($blockData) !== ChunkSnapshot::NIBBLE_BYTES) {
            throw new \ValueError('chunk terrain snapshot block-data length mismatch');
        }
        if (strlen($biomeWords) !== ChunkSnapshot::COLUMN_COUNT * 4) {
            throw new \ValueError('chunk terrain snapshot biome-word length mismatch');
        }
    }

    public function blockStateId(int $x, int $y, int $z): ?int
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

        return (ord($this->blockIds[$index]) << 4) | $data;
    }

    public function block(int $x, int $y, int $z): ?BlockState
    {
        $stateId = $this->blockStateId($x, $y, $z);

        return $stateId === null ? null : BlockState::fromId($stateId);
    }

    public function biomeColumn(int $x, int $z): ?BiomeColumn
    {
        if (!WorldBounds::containsLocal($x) || !WorldBounds::containsLocal($z)) {
            return null;
        }

        $offset = (($z << 4) | $x) * 4;
        $parts = unpack('Nword', substr($this->biomeWords, $offset, 4));

        return $parts === false ? null : BiomeColumn::fromWord($parts['word']);
    }

    public function biome(int $x, int $z): ?BiomeId
    {
        return $this->biomeColumn($x, $z)?->id;
    }

    public function biomeColor(int $x, int $z): ?int
    {
        return $this->biomeColumn($x, $z)?->color;
    }
}
