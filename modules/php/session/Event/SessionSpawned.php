<?php

declare(strict_types=1);

namespace Cobblestone\Session\Event;

final readonly class SessionSpawned
{
    public function __construct(
        public int $sessionId,
        public int $requestedRadius,
        public int $effectiveRadius,
        public int $chunksSent = 0,
        public int $encodedBytes = 0,
    ) {
    }
}
