<?php

declare(strict_types=1);

namespace Cobblestone\Internal;

use LogicException;

/** @internal */
final readonly class Protocol84BootstrapResult
{
    public const LOGIN_ACCEPTED = 'login-accepted';
    public const SPAWNED = 'spawned';
    public const GAMEPLAY = 'gameplay';

    private function __construct(
        public string $kind,
        public ?int $requestedRadius = null,
    ) {
    }

    public static function loginAccepted(): self
    {
        return new self(self::LOGIN_ACCEPTED);
    }

    public static function spawned(int $requestedRadius): self
    {
        return new self(self::SPAWNED, $requestedRadius);
    }

    public static function gameplay(): self
    {
        return new self(self::GAMEPLAY);
    }
}

/**
 * Fixed-target protocol-84 login/spawn state machine for the single-owner PHP kernel.
 *
 * Wire encoding/decoding and synthetic chunk construction stay in the native codec bridge. This
 * class owns only session ordering/state on the PHP runtime.
 */
final class Protocol84Bootstrap
{
    private const LOGIN_PACKET = 0x01;
    private const REQUEST_CHUNK_RADIUS_PACKET = 0x3d;

    private const WAIT_LOGIN = 0;
    private const WAIT_CHUNK_RADIUS = 1;
    private const SPAWNED = 2;

    /** @var array<int, int> */
    private array $states = [];

    public function __construct(
        private readonly NativeSessionRuntime $sessions,
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

    public function handle(NativeSessionPacket $packet): Protocol84BootstrapResult
    {
        $state = $this->states[$packet->sessionId] ?? null;
        if ($state === null) {
            throw new LogicException("protocol packet for unknown session {$packet->sessionId}");
        }

        if ($state === self::WAIT_LOGIN) {
            if ($packet->packetId !== self::LOGIN_PACKET) {
                throw new LogicException(
                    "expected protocol-84 Login for session {$packet->sessionId}, got packet {$packet->packetId}",
                );
            }

            $this->sessions->acceptProtocol84Login($packet->sessionId, $packet->body);
            $this->states[$packet->sessionId] = self::WAIT_CHUNK_RADIUS;

            return Protocol84BootstrapResult::loginAccepted();
        }

        if ($state === self::WAIT_CHUNK_RADIUS) {
            if ($packet->packetId !== self::REQUEST_CHUNK_RADIUS_PACKET) {
                throw new LogicException(
                    "expected RequestChunkRadius for session {$packet->sessionId}, got packet {$packet->packetId}",
                );
            }

            $requestedRadius = $this->sessions->spawnProtocol84Probe(
                $packet->sessionId,
                $packet->body,
            );
            $this->states[$packet->sessionId] = self::SPAWNED;

            return Protocol84BootstrapResult::spawned($requestedRadius);
        }

        return Protocol84BootstrapResult::gameplay();
    }
}
