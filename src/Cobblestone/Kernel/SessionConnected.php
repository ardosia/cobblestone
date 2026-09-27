<?php

declare(strict_types=1);

namespace Cobblestone\Kernel;

final readonly class SessionConnected
{
    public function __construct(
        public int $sessionId,
        public string $peer,
    ) {
    }
}
