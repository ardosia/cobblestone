<?php

declare(strict_types=1);

namespace Cobblestone\Session;

use Cobblestone\Native\Session\Packet;
use Cobblestone\Native\Session\Runtime;
use LogicException;

/** @internal */
final class JoinFlow
{
    private const LOGIN_PACKET = 0x01;
    private const REQUEST_CHUNK_RADIUS_PACKET = 0x3d;
    private const WAIT_LOGIN = 0;
    private const WAIT_CHUNK_RADIUS = 1;
    private const SPAWNED = 2;

    /** @var array<int, int> */
    private array $states = [];

    public function __construct(
        private readonly Runtime $sessions,
    ) {
    }

    public function connected(int $sessionId): void
    {
        $this->states[$sessionId] = self::WAIT_LOGIN;
    }

    public function disconnected(int $sessionId): void
    {
        unset($this->states[$sessionId]);
    }

    public function handle(Packet $packet): JoinResult
    {
        $state = $this->states[$packet->sessionId] ?? null;
        if ($state === null) {
            throw new LogicException("packet for unknown session {$packet->sessionId}");
        }

        if ($state === self::WAIT_LOGIN) {
            if ($packet->packetId !== self::LOGIN_PACKET) {
                throw new LogicException(
                    "expected Login for session {$packet->sessionId}, got packet {$packet->packetId}",
                );
            }

            $this->sessions->acceptLogin($packet->sessionId, $packet->body);
            $this->states[$packet->sessionId] = self::WAIT_CHUNK_RADIUS;
            return JoinResult::loginAccepted();
        }

        if ($state === self::WAIT_CHUNK_RADIUS) {
            if ($packet->packetId !== self::REQUEST_CHUNK_RADIUS_PACKET) {
                throw new LogicException(
                    "expected RequestChunkRadius for session {$packet->sessionId}, got packet {$packet->packetId}",
                );
            }

            $requestedRadius = $this->sessions->completeJoin($packet->sessionId, $packet->body);
            $this->states[$packet->sessionId] = self::SPAWNED;
            return JoinResult::spawned($requestedRadius);
        }

        return JoinResult::gameplay();
    }
}
