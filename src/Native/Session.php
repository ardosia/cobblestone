<?php

declare(strict_types=1);

namespace Cobblestone\Native;

use Cobblestone\Native\Session\ChunkWork;
use Cobblestone\Native\Session\ChunkWorkKind;
use Cobblestone\Native\Session\ChunkWorkResult;
use Cobblestone\Native\Session\ChunkWorkStatus;
use Cobblestone\Native\Session\Connected;
use Cobblestone\Native\Session\Disconnected;
use Cobblestone\Native\Session\LoginRequested;
use Cobblestone\Native\Session\Packet;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\Dimension;

final class Session
{
    public const DELIVERY_UNRELIABLE = 0;
    public const DELIVERY_UNRELIABLE_SEQUENCED = 1;
    public const DELIVERY_RELIABLE = 2;
    public const DELIVERY_RELIABLE_ORDERED = 3;
    public const DELIVERY_RELIABLE_SEQUENCED = 4;

    private const REQUIRED_ENTRYPOINTS = [
        'cobblestone_core_runtime_id',
        'cobblestone_session_start',
        'cobblestone_session_running',
        'cobblestone_session_poll_event',
        'cobblestone_session_send',
        'cobblestone_session_disconnect',
        'cobblestone_session_stop',
        'cobblestone_session_accept_login',
        'cobblestone_session_next_chunk_work',
        'cobblestone_session_mark_chunk_prepared',
        'cobblestone_session_complete_chunk_work',
        'cobblestone_session_flush_world_changes',
    ];

    private bool $running = false;

    private function __construct(private readonly int $runtimeId) {}

    public static function start(
        string $bind,
        int $maxConnections,
        string $serverName,
        int $maxChunkRadius,
    ): self {
        if (!extension_loaded('cobblestone_core_php')) {
            throw new \RuntimeException('cobblestone_core_php extension is not loaded');
        }
        foreach (self::REQUIRED_ENTRYPOINTS as $entrypoint) {
            if (!\function_exists($entrypoint)) {
                throw new \RuntimeException(
                    "cobblestone_core_php is stale or incompatible: missing {$entrypoint}; rebuild the extension",
                );
            }
        }

        $runtimeId = cobblestone_core_runtime_id();
        if (!is_int($runtimeId) || $runtimeId <= 0) {
            throw new \LogicException('invalid native runtime identity');
        }
        cobblestone_session_start($bind, $maxConnections, $serverName, $maxChunkRadius);

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

    public function poll(): Connected|LoginRequested|Packet|Disconnected|null
    {
        $this->assertRunning();
        $event = cobblestone_session_poll_event();
        if ($event === null) {
            return null;
        }
        if (!is_array($event) || !isset($event[0], $event[1]) || !is_string($event[0]) || !is_int($event[1])) {
            throw new \LogicException('invalid native session event envelope');
        }
        /** @var array<int, mixed> $event */

        return match ($event[0]) {
            'connected' => $this->connectedEvent($event),
            'login-requested' => $this->loginRequestedEvent($event),
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
        int $seed,
        int $generator,
        Dimension $dimension,
        int $spawnX,
        int $spawnY,
        int $spawnZ,
        int $time,
        bool $timeStarted,
        string $levelId,
    ): void {
        $this->assertRunning();
        cobblestone_session_accept_login(
            $sessionId,
            [
                $seed,
                $generator,
                $dimension->value,
                $spawnX,
                $spawnY,
                $spawnZ,
                $time,
                $timeStarted,
                $levelId,
            ],
        );
    }

    /** @internal */
    public function nextChunkWork(int $afterSessionId = 0): ?ChunkWork
    {
        $this->assertRunning();
        $work = cobblestone_session_next_chunk_work($afterSessionId);
        if ($work === null) {
            return null;
        }
        if (!is_array($work)
            || !isset($work[0], $work[1], $work[2])
            || !is_int($work[0])
            || !is_int($work[1])
            || !is_array($work[2])
            || count($work[2]) % 2 !== 0
        ) {
            throw new \LogicException('invalid native chunk-work envelope');
        }

        $positions = [];
        for ($index = 0, $count = count($work[2]); $index < $count; $index += 2) {
            if (!is_int($work[2][$index]) || !is_int($work[2][$index + 1])) {
                throw new \LogicException('invalid native chunk-work coordinate');
            }
            $positions[] = new ChunkPos($work[2][$index], $work[2][$index + 1]);
        }

        return new ChunkWork(
            $work[0],
            ChunkWorkKind::tryFrom($work[1])
                ?? throw new \LogicException('invalid native chunk-work kind'),
            $positions,
        );
    }

    /** @internal */
    public function markChunkPrepared(int $sessionId, int $worldHandle, ChunkPos $position): void
    {
        $this->assertRunning();
        cobblestone_session_mark_chunk_prepared(
            $sessionId,
            $worldHandle,
            $position->x,
            $position->z,
        );
    }

    /** @internal */
    public function completeChunkWork(int $sessionId, int $worldHandle): ChunkWorkResult
    {
        $this->assertRunning();
        $result = cobblestone_session_complete_chunk_work($sessionId, $worldHandle);
        if (!is_array($result) || count($result) !== 6) {
            throw new \LogicException('invalid native chunk-work completion envelope');
        }
        foreach ($result as $value) {
            if (!is_int($value)) {
                throw new \LogicException('invalid native chunk-work completion value');
            }
        }

        return new ChunkWorkResult(
            ChunkWorkStatus::tryFrom($result[0])
                ?? throw new \LogicException('invalid native chunk-work completion status'),
            $result[1],
            $result[2],
            $result[3],
            $result[4],
            $result[5],
        );
    }

    /** @internal Returns the number of viewer batches queued this tick. */
    public function flushWorldChanges(int $worldHandle): int
    {
        $this->assertRunning();
        $queued = cobblestone_session_flush_world_changes($worldHandle);
        if (!is_int($queued)) {
            throw new \LogicException('invalid native world-change flush result');
        }
        return $queued;
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
        if (!isset($event[1], $event[2]) || !is_int($event[1]) || !is_string($event[2])) {
            throw new \LogicException('invalid native connected event');
        }
        return new Connected($event[1], $event[2]);
    }

    /** @param array<int, mixed> $event */
    private function loginRequestedEvent(array $event): LoginRequested
    {
        if (!isset($event[1]) || !is_int($event[1])) {
            throw new \LogicException('invalid native login-requested event');
        }
        return new LoginRequested($event[1]);
    }

    /** @param array<int, mixed> $event */
    private function packetEvent(array $event): Packet
    {
        if (!isset($event[1], $event[2], $event[3]) || !is_int($event[1]) || !is_int($event[2]) || !is_string($event[3])) {
            throw new \LogicException('invalid native packet event');
        }
        return new Packet($event[1], $event[2], $event[3]);
    }

    /** @param array<int, mixed> $event */
    private function disconnectedEvent(array $event): Disconnected
    {
        if (!isset($event[1], $event[2]) || !is_int($event[1]) || !is_string($event[2])) {
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
