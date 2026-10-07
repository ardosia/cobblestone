<?php

declare(strict_types=1);

namespace Cobblestone\Session\Internal;

/** @internal */
final readonly class SpawnCompletion
{
    public function __construct(
        public int $sessionId,
        public int $requestedRadius,
        public int $effectiveRadius,
        public int $chunksSent,
        public int $encodedBytes,
        public int $chunkEncodeNanos,
    ) {}
}
