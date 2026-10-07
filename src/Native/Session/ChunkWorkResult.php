<?php

declare(strict_types=1);

namespace Cobblestone\Native\Session;

/** @internal */
final readonly class ChunkWorkResult
{
    public function __construct(
        public ChunkWorkStatus $status,
        public int $requestedRadius = 0,
        public int $effectiveRadius = 0,
        public int $chunksSent = 0,
        public int $encodedBytes = 0,
        public int $encodeNanos = 0,
    ) {}
}
