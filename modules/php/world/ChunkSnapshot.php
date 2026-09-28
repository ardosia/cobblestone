<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Immutable semantic snapshot used at bulk native/wire boundaries.
 *
 * The planes are gameplay/world state, not a protocol packet. Block and nibble planes use
 * global Y/Z/X order, which also matches protocol-84 ORDER_LAYERED terrain planes.
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
        public int $lightRevision = 0,
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
        if ($lightRevision < 0) {
            throw new \ValueError('chunk light snapshot revision cannot be negative');
        }
    }

    public function position(): ChunkPos
    {
        return $this->position;
    }

    public function revision(): ChunkRevision
    {
        return new ChunkRevision($this->revision);
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

    public function skyLightLevel(int $x, int $y, int $z): ?int
    {
        return $this->lightLevel($this->skyLight, $x, $y, $z);
    }

    public function blockLightLevel(int $x, int $y, int $z): ?int
    {
        return $this->lightLevel($this->blockLight, $x, $y, $z);
    }

    public function biomeId(int $x, int $z): ?int
    {
        if (!WorldBounds::containsLocal($x) || !WorldBounds::containsLocal($z)) {
            return null;
        }

        return ord($this->biomes[($z << 4) | $x]);
    }

    public function blockExtraDataAt(int $x, int $y, int $z): ?int
    {
        if (
            !WorldBounds::containsLocal($x)
            || !WorldBounds::containsLocal($z)
            || !WorldBounds::containsY($y)
        ) {
            return null;
        }

        return $this->extraData[($y << 8) | ($z << 4) | $x] ?? 0;
    }

    public function terrain(): ChunkDataSnapshot
    {
        return new ChunkDataSnapshot($this->blockIds, $this->blockData, $this->biomes);
    }

    public function light(): LightSnapshot
    {
        return new LightSnapshot(
            new LightRevision($this->lightRevision),
            $this->skyLight,
            $this->blockLight,
        );
    }

    private function lightLevel(string $plane, int $x, int $y, int $z): ?int
    {
        if (
            !WorldBounds::containsLocal($x)
            || !WorldBounds::containsLocal($z)
            || !WorldBounds::containsY($y)
        ) {
            return null;
        }

        $index = ($y << 8) | ($z << 4) | $x;
        $byte = ord($plane[$index >> 1]);

        return ($index & 1) === 0 ? $byte & 0x0f : ($byte >> 4) & 0x0f;
    }
}
