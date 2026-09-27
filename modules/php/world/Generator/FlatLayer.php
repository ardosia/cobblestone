<?php

declare(strict_types=1);

namespace Cobblestone\World\Generator;

use Cobblestone\World\BlockState;
use ValueError;

final readonly class FlatLayer
{
    public function __construct(
        public int $count,
        public BlockState $state,
    ) {
        if ($count <= 0) {
            throw new ValueError('flat layer count must be positive');
        }
    }
}
