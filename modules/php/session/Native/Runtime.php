<?php

declare(strict_types=1);

namespace Cobblestone\Native\Session;

final class Runtime
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

    public static function start(string $bind, int $maxConnections, string $serverName): self
    {
        if (!extension_loaded('cobblestone_core_php')) {
            throw new \RuntimeException('cobblestone_core_php extension is not loaded');
        }

        foreach ([
            'cobblestone_core_runtime_id',
            'cobblestone_session_start',
            'cobblestone_session_running',
            'cobblestone_session_poll_event',
            'cobblestone_session_send',
            'cobblestone_session_protocol84_accept_login',
            'cobblestone_session_protocol84_spawn_probe',
            'cobblestone_session_protocol84_accept_login_world',
            'cobblestone_session_protocol84_request_chunk_radius',
            'cobblestone_session_protocol84_send_initial_chunks',
            'cobblestone_session_protocol84_send_native_chunks',
            'cobblestone_session_protocol84_player_spawned',
            'cobblestone_session_protocol84_track_move_player',
            'cobblestone_session_protocol84_flush_world_changes',
            'cobblestone_session_disconnect',
            'cobblestone_session_stop',
        ] as $function) {
            if (!\function_exists($function)) {
                throw new \RuntimeException(
                    "cobblestone_core_php is stale or incompatible: missing native function {$function}; rebuild the extension",
                );
            }
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
        return $this->running;
    }

    public function poll(): Connected|Packet|Disconnected|null
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

    /** @internal */
    public function acceptLogin(
        int $sessionId,
        string $body,
        int $seed,
        int $generator,
        int $spawnX,
        int $spawnY,
        int $spawnZ,
        int $time,
        bool $timeStarted,
        string $levelId,
    ): void {
        $this->assertRunning();
        cobblestone_session_protocol84_accept_login_world(
            $sessionId,
            $body,
            $seed,
            $generator,
            $spawnX,
            $spawnY,
            $spawnZ,
            $time,
            $timeStarted,
            $levelId,
        );
    }

    /** @internal */
    public function requestedChunkRadius(string $body): int
    {
        $this->assertRunning();
        return cobblestone_session_protocol84_request_chunk_radius($body);
    }

    /** @internal */
    public function initializePlayerPosition(int $sessionId, int $spawnX, int $spawnY, int $spawnZ): void
    {
        $this->assertRunning();
        cobblestone_session_protocol84_player_spawned($sessionId, $spawnX, $spawnY, $spawnZ);
    }

    /** @internal */
    public function trackPlayerMovement(int $sessionId, string $body): string
    {
        $this->assertRunning();
        return cobblestone_session_protocol84_track_move_player($sessionId, $body);
    }

    /** @internal */
    public function sendInitialChunks(int $sessionId, int $effectiveRadius, string $projection): int
    {
        $this->assertRunning();
        return cobblestone_session_protocol84_send_initial_chunks(
            $sessionId,
            $effectiveRadius,
            $projection,
        );
    }


    /** @internal */
    public function sendInitialWorldChunks(
        int $sessionId,
        int $effectiveRadius,
        int $worldHandle,
        int $centerChunkX,
        int $centerChunkZ,
    ): int {
        $this->assertRunning();
        return cobblestone_session_protocol84_send_native_chunks(
            $sessionId,
            $effectiveRadius,
            $worldHandle,
            $centerChunkX,
            $centerChunkZ,
        );
    }

    /** @internal Returns the number of viewer batches queued this tick. */
    public function flushWorldChanges(int $worldHandle): int
    {
        $this->assertRunning();

        return cobblestone_session_protocol84_flush_world_changes($worldHandle);
    }

    public function disconnect(int $sessionId): void
    {
        $this->assertRunning();
        cobblestone_session_disconnect($sessionId);
    }

    public function stop(): void
    {
        if (!$this->running) {
            return;
        }

        cobblestone_session_stop();
        $this->running = false;
    }

    /** @param array<int, mixed> $event */
    private function connectedEvent(array $event): Connected
    {
        if (!isset($event[2]) || !is_string($event[2])) {
            throw new \LogicException('invalid native connected event');
        }
        return new Connected($event[1], $event[2]);
    }

    /** @param array<int, mixed> $event */
    private function packetEvent(array $event): Packet
    {
        if (!isset($event[2], $event[3]) || !is_int($event[2]) || !is_string($event[3])) {
            throw new \LogicException('invalid native packet event');
        }
        return new Packet($event[1], $event[2], $event[3]);
    }

    /** @param array<int, mixed> $event */
    private function disconnectedEvent(array $event): Disconnected
    {
        if (!isset($event[2]) || !is_string($event[2])) {
            throw new \LogicException('invalid native disconnected event');
        }
        return new Disconnected($event[1], $event[2]);
    }

    private function assertRunning(): void
    {
        if (!$this->running) {
            throw new \LogicException('native session runtime is not running');
        }
    }
}
