<?php

declare(strict_types=1);

namespace Cobblestone\Internal;

/**
 * Kernel-internal event emitted when a native gameplay session is accepted.
 *
 * This is not a plugin API. It intentionally exposes session identity rather than RakNet objects.
 */
final readonly class NativeSessionConnected
{
    public function __construct(
        public int $sessionId,
        public string $peer,
    ) {
    }
}

/**
 * Kernel-internal protocol-84 packet event.
 *
 * Packet IDs/bodies are wire data consumed by the server kernel and must not leak into ordinary
 * gameplay/plugin APIs.
 */
final readonly class NativeSessionPacket
{
    public function __construct(
        public int $sessionId,
        public int $packetId,
        public string $body,
    ) {
    }
}

/**
 * Kernel-internal event emitted when the native session mechanism closes a peer.
 */
final readonly class NativeSessionDisconnected
{
    public function __construct(
        public int $sessionId,
        public string $reason,
    ) {
    }
}

/**
 * Single-owner PHP facade over the native session mechanism.
 *
 * Networking and native concurrency live below this object. All methods execute on the owning PHP
 * runtime; native threads communicate back only through the bounded event queue exposed by poll().
 */
final class NativeSessionRuntime
{
    public const DELIVERY_UNRELIABLE = 0;
    public const DELIVERY_UNRELIABLE_SEQUENCED = 1;
    public const DELIVERY_RELIABLE = 2;
    public const DELIVERY_RELIABLE_ORDERED = 3;
    public const DELIVERY_RELIABLE_SEQUENCED = 4;

    private bool $running = false;

    private function __construct(
        private readonly int $runtimeId,
    ) {
    }

    public static function start(
        string $bind,
        int $maxConnections,
        string $serverName,
    ): self {
        if (!extension_loaded('cobblestone_core_php')) {
            throw new \RuntimeException('cobblestone_core_php extension is not loaded');
        }

        $runtimeId = cobblestone_core_runtime_id();
        cobblestone_session_start($bind, $maxConnections, $serverName);

        $runtime = new self($runtimeId);
        $runtime->running = true;

        return $runtime;
    }

    public function runtimeId(): int
    {
        return $this->runtimeId;
    }

    public function isRunning(): bool
    {
        $this->assertOwner();

        return $this->running && cobblestone_session_running();
    }

    public function poll(): NativeSessionConnected|NativeSessionPacket|NativeSessionDisconnected|null
    {
        $this->assertRunning();

        $event = cobblestone_session_poll_event();
        if ($event === null) {
            return null;
        }

        if (!is_array($event) || !isset($event[0], $event[1]) || !is_string($event[0]) || !is_int($event[1])) {
            throw new \LogicException('invalid native session event envelope');
        }

        return match ($event[0]) {
            'connected' => $this->connectedEvent($event),
            'packet' => $this->packetEvent($event),
            'disconnected' => $this->disconnectedEvent($event),
            default => throw new \LogicException('unknown native session event kind'),
        };
    }

    public function send(
        int $sessionId,
        int $packetId,
        string $body,
        int $delivery = self::DELIVERY_RELIABLE_ORDERED,
    ): void {
        $this->assertRunning();
        cobblestone_session_send($sessionId, $packetId, $body, $delivery);
    }

    public function disconnect(int $sessionId): void
    {
        $this->assertRunning();
        cobblestone_session_disconnect($sessionId);
    }

    public function stop(): void
    {
        $this->assertOwner();

        if (!$this->running) {
            return;
        }

        cobblestone_session_stop();
        $this->running = false;
    }

    /**
     * @param array<int, mixed> $event
     */
    private function connectedEvent(array $event): NativeSessionConnected
    {
        if (!isset($event[2]) || !is_string($event[2])) {
            throw new \LogicException('invalid native connected event');
        }

        return new NativeSessionConnected($event[1], $event[2]);
    }

    /**
     * @param array<int, mixed> $event
     */
    private function packetEvent(array $event): NativeSessionPacket
    {
        if (!isset($event[2], $event[3]) || !is_int($event[2]) || !is_string($event[3])) {
            throw new \LogicException('invalid native packet event');
        }

        return new NativeSessionPacket($event[1], $event[2], $event[3]);
    }

    /**
     * @param array<int, mixed> $event
     */
    private function disconnectedEvent(array $event): NativeSessionDisconnected
    {
        if (!isset($event[2]) || !is_string($event[2])) {
            throw new \LogicException('invalid native disconnected event');
        }

        return new NativeSessionDisconnected($event[1], $event[2]);
    }

    private function assertRunning(): void
    {
        $this->assertOwner();

        if (!$this->running || !cobblestone_session_running()) {
            throw new \LogicException('native session runtime is not running');
        }
    }

    private function assertOwner(): void
    {
        if (cobblestone_core_runtime_id() !== $this->runtimeId) {
            throw new \LogicException('native session runtime used from the wrong PHP runtime');
        }
    }
}
