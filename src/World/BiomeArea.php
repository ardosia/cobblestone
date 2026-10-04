<?php

declare(strict_types=1);

namespace Cobblestone\World;

use OutOfBoundsException;
use ValueError;

/**
 * Immutable sampled biome-ID plane.
 *
 * The compact byte plane is retained internally; typed BiomeId/BiomeColumn values are materialized
 * only when requested by semantic PHP code.
 */
final readonly class BiomeArea
{
    private function __construct(
        public int $originX,
        public int $originZ,
        public int $width,
        public int $height,
        private string $ids,
    ) {
    }

    public static function fromBinary(
        int $originX,
        int $originZ,
        int $width,
        int $height,
        string $ids,
    ): self {
        if ($width <= 0 || $height <= 0) {
            throw new ValueError('biome area dimensions must be positive');
        }

        $expected = $width * $height;
        if (strlen($ids) !== $expected) {
            throw new ValueError(
                "biome area byte length mismatch: expected {$expected}, got " . strlen($ids),
            );
        }

        for ($index = 0; $index < $expected; ++$index) {
            $id = ord($ids[$index]);
            if (!BiomeCatalog::supports($id)) {
                throw new ValueError("biome area contains unsupported MCPE 0.15.10 biome id {$id}");
            }
        }

        return new self($originX, $originZ, $width, $height, $ids);
    }

    public function idAt(int $localX, int $localZ): BiomeId
    {
        return new BiomeId(ord($this->ids[$this->index($localX, $localZ)]));
    }

    public function columnAt(int $localX, int $localZ): BiomeColumn
    {
        return $this->idAt($localX, $localZ)->column();
    }

    /** @internal Compact Z/X-ordered biome IDs for native/generation boundaries. */
    public function binaryIds(): string
    {
        return $this->ids;
    }

    private function index(int $localX, int $localZ): int
    {
        if (
            $localX < 0 || $localX >= $this->width
            || $localZ < 0 || $localZ >= $this->height
        ) {
            throw new OutOfBoundsException(
                "biome area coordinate {$localX}:{$localZ} is outside {$this->width}x{$this->height}",
            );
        }

        return $localX + $localZ * $this->width;
    }
}
