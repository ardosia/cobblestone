<?php

declare(strict_types=1);

namespace Cobblestone\World;

final readonly class BlockPos
{
    public function __construct(
        public int $x,
        public int $y,
        public int $z,
    ) {
    }

    public function chunk(): ChunkPos
    {
        return ChunkPos::fromBlock($this->x, $this->z);
    }

    public function sectionY(): ?SectionY
    {
        return SectionY::fromBlockY($this->y);
    }

    public function localX(): int
    {
        return ChunkPos::localCoordinate($this->x);
    }

    public function localY(): ?int
    {
        return WorldBounds::containsY($this->y)
            ? $this->y % WorldBounds::SECTION_EDGE
            : null;
    }

    public function localZ(): int
    {
        return ChunkPos::localCoordinate($this->z);
    }

    public function isInsideWorld(): bool
    {
        return WorldBounds::containsY($this->y);
    }
}
