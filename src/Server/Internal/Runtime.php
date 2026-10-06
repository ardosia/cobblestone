<?php

declare(strict_types=1);

namespace Cobblestone\Server\Internal;

use Closure;
use Cobblestone\Event\Internal\Dispatcher;
use Cobblestone\Native\Session\Connected;
use Cobblestone\Native\Session\Disconnected;
use Cobblestone\Native\Session\Packet;
use Cobblestone\Native\Session as NativeSessions;
use Cobblestone\Server\ServerConfig;
use Cobblestone\Session\Internal\BootstrapUpdate;
use Cobblestone\Session\Event\SessionConnected;
use Cobblestone\Session\Event\SessionDisconnected;
use Cobblestone\Session\Event\SessionLoginAccepted;
use Cobblestone\Session\Event\SessionSpawned;
use Cobblestone\Session\Internal\Bootstrap;
use Cobblestone\Session\Internal\Gameplay;
use Cobblestone\Task\Scheduler;
use Cobblestone\World\World;
use LogicException;
use Psr\Log\LoggerInterface;
use Throwable;

/**
 * @internal Owner-runtime orchestration behind the public Server facade.
 */
final class Runtime
{
    private const CHUNK_EVICTION_BUDGET = 64;

    private function __construct(
        private readonly NativeSessions $sessions,
        private readonly Bootstrap $bootstrap,
        private readonly Gameplay $gameplay,
        private readonly World $world,
        private readonly Dispatcher $events,
        private readonly Scheduler $scheduler,
        private readonly LoggerInterface $logger,
        private readonly ?Closure $packetHandler,
    ) {}

    /** @param Closure(Packet): void|null $packetHandler */
    public static function start(
        ServerConfig $config,
        World $world,
        Dispatcher $events,
        Scheduler $scheduler,
        LoggerInterface $logger,
        ?Closure $packetHandler,
    ): self {
        $sessions = NativeSessions::start(
            $config->bind,
            $config->maxConnections,
            $config->name,
        );

        try {
            return new self(
                $sessions,
                new Bootstrap($sessions, $world, $config->initialChunkRadius),
                new Gameplay($sessions, $world, $config->initialChunkRadius),
                $world,
                $events,
                $scheduler,
                $logger,
                $packetHandler,
            );
        } catch (Throwable $error) {
            try {
                $sessions->stop();
            } catch (Throwable) {
            }

            throw $error;
        }
    }

    public function running(): bool
    {
        return $this->sessions->isRunning();
    }

    public function tick(int $nativeEventBudget): void
    {
        if ($nativeEventBudget <= 0) {
            throw new LogicException('native event budget must be positive');
        }

        $this->scheduler->tick();
        $this->pollSessions($nativeEventBudget);
        $this->tickStorage();

        foreach ($this->bootstrap->tick() as $completion) {
            $this->dispatchSpawned($completion['sessionId'], $completion['update']);
        }

        $this->gameplay->tick();
        $this->flushWorldChanges();
    }

    public function stopGameplay(): void
    {
        $this->gameplay->stop();
    }

    public function stopSessions(): void
    {
        $this->sessions->stop();
    }

    /**
     * Mirrors the pre-extraction start failure cleanup: native session shutdown is the required
     * rollback once Server::start() has crossed the native start boundary.
     */
    public function abortStart(): void
    {
        try {
            $this->sessions->stop();
        } catch (Throwable) {
        }
    }

    private function pollSessions(int $budget): void
    {
        for ($processed = 0; $processed < $budget; ++$processed) {
            $event = $this->sessions->poll();
            if ($event === null) {
                return;
            }

            if ($event instanceof Connected) {
                $this->connected($event);
                continue;
            }
            if ($event instanceof Disconnected) {
                $this->disconnected($event);
                continue;
            }
            if ($event instanceof Packet) {
                $this->packet($event);
            }
        }
    }

    private function connected(Connected $event): void
    {
        $this->bootstrap->connected($event->sessionId);
        $this->logger->info(
            'Session connected',
            ['session' => $event->sessionId, 'peer' => $event->peer],
        );
        $this->events->dispatch(new SessionConnected($event->sessionId, $event->peer));
    }

    private function disconnected(Disconnected $event): void
    {
        $this->bootstrap->disconnected($event->sessionId);
        $this->gameplay->disconnected($event->sessionId);
        $this->logger->info(
            'Session disconnected',
            ['session' => $event->sessionId, 'reason' => $event->reason],
        );
        $this->events->dispatch(new SessionDisconnected($event->sessionId, $event->reason));
    }

    private function packet(Packet $packet): void
    {
        try {
            $update = $this->bootstrap->handle($packet);
        } catch (Throwable $error) {
            $this->logger->error(
                'Session bootstrap failed',
                [
                    'session' => $packet->sessionId,
                    'packet' => $packet->packetId,
                    'exception' => $error,
                ],
            );
            $this->disconnectAfterFailure(
                $packet->sessionId,
                'Session disconnect after bootstrap failure failed',
            );
            return;
        }

        if ($update->kind === BootstrapUpdate::LOGIN_ACCEPTED) {
            $this->logger->info(
                'Session login accepted',
                ['session' => $packet->sessionId, 'protocol' => 84],
            );
            $this->events->dispatch(new SessionLoginAccepted($packet->sessionId));
            return;
        }
        if ($update->kind === BootstrapUpdate::CHUNKS_LOADING) {
            return;
        }
        if ($update->kind === BootstrapUpdate::SPAWNED) {
            $this->dispatchSpawned($packet->sessionId, $update);
            return;
        }

        try {
            $this->gameplay->handle($packet);
        } catch (Throwable $error) {
            $this->logger->error(
                'Session gameplay packet failed',
                [
                    'session' => $packet->sessionId,
                    'packet' => $packet->packetId,
                    'exception' => $error,
                ],
            );
            $this->disconnectAfterFailure(
                $packet->sessionId,
                'Session disconnect after gameplay failure failed',
            );
            return;
        }

        if ($this->packetHandler !== null) {
            ($this->packetHandler)($packet);
        }
    }

    private function disconnectAfterFailure(int $sessionId, string $message): void
    {
        try {
            $this->sessions->disconnect($sessionId);
        } catch (Throwable $disconnectError) {
            $this->logger->warning(
                $message,
                [
                    'session' => $sessionId,
                    'exception' => $disconnectError,
                ],
            );
        }
    }

    private function dispatchSpawned(int $sessionId, BootstrapUpdate $update): void
    {
        $this->gameplay->spawned($sessionId);
        $effectiveRadius = $update->effectiveRadius ?? 0;
        $this->logger->info(
            'Session spawned',
            [
                'session' => $sessionId,
                'requested_radius' => $update->requestedRadius ?? 0,
                'initial_radius' => $effectiveRadius,
                'chunks_sent' => $update->chunksSent,
                'encoded_bytes' => $update->encodedBytes,
                'chunk_encode_ms' => round($update->chunkEncodeNanos / 1_000_000, 3),
            ],
        );
        $this->events->dispatch(
            new SessionSpawned(
                $sessionId,
                $update->requestedRadius ?? 0,
                $effectiveRadius,
                $update->chunksSent,
                $update->encodedBytes,
            ),
        );
    }

    private function tickStorage(): void
    {
        $native = $this->world->nativeStore();
        if ($native === null || !$native->hasStorage()) {
            return;
        }

        $storageTick = $native->storageTick(64);
        if ($storageTick['compaction_failed'] === 0) {
            return;
        }

        $storageStats = $native->storageStats();
        $this->logger->warning(
            'Persistent region compaction failed; automatic retry is blocked for this process',
            [
                'failures' => $storageTick['compaction_failed'],
                'blocked_regions' => $storageStats['compaction_blocked_regions'],
                'last_error' => $storageStats['compaction_last_error'],
            ],
        );
    }

    private function flushWorldChanges(): void
    {
        $native = $this->world->nativeStore();
        if ($native === null) {
            return;
        }

        $this->sessions->flushWorldChanges($native->handle());
        if ($native->hasStorage()) {
            $this->world->chunks()->evictCleanUnpinned(self::CHUNK_EVICTION_BUDGET);
        }
    }
}
