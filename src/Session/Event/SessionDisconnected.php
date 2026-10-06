<?php

declare(strict_types=1);

namespace Cobblestone\Session\Event;

final readonly class SessionDisconnected
{
    public function __construct(
        public int $sessionId,
        public string $reason,
    ) {}
}
