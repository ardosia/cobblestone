<?php

declare(strict_types=1);

namespace Cobblestone\Kernel;

final readonly class SessionSpawned
{
    public function __construct(
        public int $sessionId,
        public int $requestedRadius,
        public int $probeRadius,
    ) {
    }
}
