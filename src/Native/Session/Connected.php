<?php

declare(strict_types=1);

namespace Cobblestone\Native\Session;

/** @internal */
final readonly class Connected
{
    public function __construct(
        public int $sessionId,
        public string $peer,
    ) {}
}
