<?php

declare(strict_types=1);

namespace Cobblestone\Internal;

/** @internal */
final readonly class NativeSessionPacket
{
    public function __construct(
        public int $sessionId,
        public int $packetId,
        public string $body,
    ) {
    }
}
