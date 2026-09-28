<?php

declare(strict_types=1);

namespace Cobblestone\Session;

use Cobblestone\Native\Session\Packet;
use Cobblestone\Native\Session\Runtime;
use Cobblestone\World\World;

/** @internal */
final readonly class SessionGameplay
{
    private const MOVE_PLAYER_PACKET = 0x10;

    public function __construct(
        private Runtime $sessions,
        private World $world,
    ) {
    }

    public function spawned(int $sessionId): void
    {
        $spawn = $this->world->spawn();
        $this->sessions->initializePlayerPosition(
            $sessionId,
            $spawn->x,
            $spawn->y,
            $spawn->z,
        );
    }

    public function handle(Packet $packet): void
    {
        if ($packet->packetId !== self::MOVE_PLAYER_PACKET) {
            return;
        }

        $this->sessions->trackPlayerMovement($packet->sessionId, $packet->body);
    }
}
