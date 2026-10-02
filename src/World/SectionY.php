<?php

declare(strict_types=1);

namespace Cobblestone\World;

use ValueError;

/** Fixed-target section-Y identity in range 0..7. */
final readonly class SectionY
{
    public function __construct(public int $value)
    {
        if ($value < 0 || $value >= WorldBounds::SECTION_COUNT) {
            throw new ValueError('section y must be in range 0..7');
        }
    }

    public static function fromBlockY(int $y): ?self
    {
        if (!WorldBounds::containsY($y)) {
            return null;
        }

        return new self(intdiv($y, WorldBounds::SECTION_EDGE));
    }

    public function index(): int
    {
        return $this->value;
    }

    public function minBlockY(): int
    {
        return $this->value * WorldBounds::SECTION_EDGE;
    }

    public function maxBlockY(): int
    {
        return $this->minBlockY() + WorldBounds::SECTION_EDGE - 1;
    }
}
