<?php

declare(strict_types=1);

namespace Cobblestone\Internal;

/** @internal */
final readonly class NativeSessionConnected
{
    public function __construct(
        public int $sessionId,
        public string $peer,
    ) {
    }
}
