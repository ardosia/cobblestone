<?php

declare(strict_types=1);

namespace Cobblestone\Session\Event;

final readonly class SessionConnected
{
    public function __construct(
        public int $sessionId,
        public string $peer,
    ) {}
}
