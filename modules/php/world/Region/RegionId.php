<?php

declare(strict_types=1);

namespace Cobblestone\World\Region;

final readonly class RegionId
{
    public function __construct(
        public int $x,
        public int $z,
    ) {
    }

    public function key(): string
    {
        return $this->x . ':' . $this->z;
    }
}
