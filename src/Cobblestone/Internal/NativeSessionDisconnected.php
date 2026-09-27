<?php

declare(strict_types=1);

namespace Cobblestone\Internal;

/** @internal */
final readonly class NativeSessionDisconnected
{
    public function __construct(
        public int $sessionId,
        public string $reason,
    ) {
    }
}
