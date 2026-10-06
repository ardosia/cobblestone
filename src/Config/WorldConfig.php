<?php

declare(strict_types=1);

namespace Cobblestone\Config;

use InvalidArgumentException;

final readonly class WorldConfig
{
    public function __construct(
        public string $directory,
        public string $name = 'Cobblestone',
        public int $seed = -1,
    ) {
        if (trim($directory) === '') {
            throw new InvalidArgumentException('world directory must not be empty');
        }
        if (trim($name) === '') {
            throw new InvalidArgumentException('world name must not be empty');
        }
    }
}
