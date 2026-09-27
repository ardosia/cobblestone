<?php

declare(strict_types=1);

namespace Cobblestone\Kernel;

final readonly class SessionLoginAccepted
{
    public function __construct(
        public int $sessionId,
    ) {
    }
}
