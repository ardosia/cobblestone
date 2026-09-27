<?php

declare(strict_types=1);

namespace Cobblestone\World;

use ValueError;

final readonly class BiomeId
{
    public function __construct(public int $value)
    {
        if ($value < 0 || $value > 0xff) {
            throw new ValueError('fixed-target biome id must be in range 0..255');
        }
    }
}
