<?php

declare(strict_types=1);

namespace Cobblestone\Config;

use InvalidArgumentException;

final readonly class ServerConfig
{
    public function __construct(
        public string $bind = '0.0.0.0:19132',
        public int $maxConnections = 20,
        public string $name = 'Cobblestone',
        public int $initialChunkRadius = 3,
        public int $tickRate = 20,
    ) {
        if (trim($bind) === '') {
            throw new InvalidArgumentException('server bind address must not be empty');
        }
        if ($maxConnections <= 0) {
            throw new InvalidArgumentException('max connections must be positive');
        }
        if (trim($name) === '') {
            throw new InvalidArgumentException('server name must not be empty');
        }
        if ($initialChunkRadius < 1 || $initialChunkRadius > 3) {
            throw new InvalidArgumentException('initial chunk radius must be in range 1..3');
        }
        if ($tickRate <= 0 || $tickRate > 1_000) {
            throw new InvalidArgumentException('tick rate must be in range 1..1000');
        }
    }
}
