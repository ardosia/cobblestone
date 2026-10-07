<?php

declare(strict_types=1);

namespace Cobblestone\Server\Internal;

use Cobblestone\Internal\Target;
use Closure;
use Cobblestone\Event\Internal\Dispatcher;
use Cobblestone\Native\Session\Connected;
use Cobblestone\Native\Session\Disconnected;
use Cobblestone\Native\Session\LoginRequested;
use Cobblestone\Native\Session\Packet;
use Cobblestone\Native\Session as NativeSessions;
use Cobblestone\Config\ServerConfig;
use Cobblestone\Session\Event\SessionConnected;
use Cobblestone\Session\Event\SessionDisconnected;
use Cobblestone\Session\Event\SessionLoginAccepted;
use Cobblestone\Session\Event\SessionSpawned;
use Cobblestone\Session\Internal\Coordinator;
use Cobblestone\Session\Internal\SpawnCompletion;
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
        private readonly Coordinator $coordinator,
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
            $config->initialChunkRadius,
        );

        try {
            return new self(
                $sessions,
                new Coordinator($sessions, $world),
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

        foreach ($this->coordinator->tick() as $completion) {
            $this->dispatchSpawned($completion);
        }

        $this->flushWorldChanges();
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
            if ($event instanceof LoginRequested) {
                $this->loginRequested($event);
                continue;
            }
            if ($event instanceof Packet) {
                $this->packet($event);
            }
        }
    }

    private function connected(Connected $event): void
    {
        $this->logger->info(
            'Session connected',
            ['session' => $event->sessionId, 'peer' => $event->peer],
        );
        $this->events->dispatch(new SessionConnected($event->sessionId, $event->peer));
    }

    private function disconnected(Disconnected $event): void
    {
        $this->logger->info(
            'Session disconnected',
            ['session' => $event->sessionId, 'reason' => $event->reason],
        );
        $this->events->dispatch(new SessionDisconnected($event->sessionId, $event->reason));
    }

    private function loginRequested(LoginRequested $event): void
    {
        try {
            $this->coordinator->acceptLogin($event->sessionId);
        } catch (Throwable $error) {
            $this->logger->error(
                'Session login acceptance failed',
                ['session' => $event->sessionId, 'exception' => $error],
            );
            $this->disconnectAfterFailure(
                $event->sessionId,
                'Session disconnect after login failure failed',
            );
            return;
        }

        $this->logger->info(
            'Session login accepted',
            ['session' => $event->sessionId, 'protocol' => Target::GAME_PROTOCOL],
        );
        $this->events->dispatch(new SessionLoginAccepted($event->sessionId));
    }

    private function packet(Packet $packet): void
    {
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

    private function dispatchSpawned(SpawnCompletion $completion): void
    {
        $this->logger->info(
            'Session spawned',
            [
                'session' => $completion->sessionId,
                'requested_radius' => $completion->requestedRadius,
                'initial_radius' => $completion->effectiveRadius,
                'chunks_sent' => $completion->chunksSent,
                'encoded_bytes' => $completion->encodedBytes,
                'chunk_encode_ms' => round($completion->chunkEncodeNanos / 1_000_000, 3),
            ],
        );
        $this->events->dispatch(
            new SessionSpawned(
                $completion->sessionId,
                $completion->requestedRadius,
                $completion->effectiveRadius,
                $completion->chunksSent,
                $completion->encodedBytes,
            ),
        );
    }

    private function tickStorage(): void
    {
        $native = $this->world->nativeStore();
        if (!$native->hasStorage()) {
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
        $this->sessions->flushWorldChanges($native->handle());
        if ($native->hasStorage()) {
            $this->world->chunks()->evictCleanUnpinned(self::CHUNK_EVICTION_BUDGET);
        }
    }
}
