<?php

declare(strict_types=1);

namespace Cobblestone\Native\Session;

/** @internal */
final readonly class Packet
{
    public function __construct(
        public int $sessionId,
        public int $packetId,
        public string $body,
    ) {}
}
