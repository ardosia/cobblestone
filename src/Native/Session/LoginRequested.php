<?php

declare(strict_types=1);

namespace Cobblestone\Native\Session;

/** @internal */
final readonly class LoginRequested
{
    public function __construct(public int $sessionId) {}
}
