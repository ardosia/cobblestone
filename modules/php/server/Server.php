<?php

declare(strict_types=1);

namespace Cobblestone\Server;

use Closure;
use Cobblestone\Command\CommandRegistry;
use Cobblestone\Event\EventBus;
use Cobblestone\Log\LoggerFactory;
use Cobblestone\Native\Session\Connected;
use Cobblestone\Native\Session\Disconnected;
use Cobblestone\Native\Session\Packet;
use Cobblestone\Native\Session\Runtime;
use Cobblestone\Plugin\PluginManager;
use Cobblestone\Server\Event\ServerStarted;
use Cobblestone\Server\Event\ServerStopping;
use Cobblestone\Session\Event\SessionConnected;
use Cobblestone\Session\Event\SessionDisconnected;
use Cobblestone\Session\Event\SessionLoginAccepted;
use Cobblestone\Session\Event\SessionSpawned;
use Cobblestone\Session\SessionBootstrap;
use Cobblestone\Session\SessionGameplay;
use Cobblestone\Session\BootstrapUpdate;
use Cobblestone\Task\Scheduler;
use Cobblestone\World\ChunkLoadPending;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\Generator\FlatGenerator;
use Cobblestone\World\ResidentChunkHandle;
use Cobblestone\World\World;
use LogicException;
use Psr\Log\LoggerInterface;
use Throwable;

final class Server
{
    private readonly EventBus $events;
    private readonly CommandRegistry $commands;
    private readonly Scheduler $scheduler;
    private readonly PluginManager $plugins;
    private readonly SessionBootstrap $bootstrap;
    private readonly SessionGameplay $gameplay;
    private readonly WorldMaintenance $worldMaintenance;
    private readonly LoggerInterface $logger;

    private ServerState $state = ServerState::Starting;
    private ?string $stopReason = null;

    /** @param Closure(Packet): void|null $packetHandler */
    private function __construct(
        private readonly Runtime $sessions,
        private readonly LoggerFactory $logs,
        private ?Closure $packetHandler,
        private readonly string $serverName,
        private readonly World $world,
        int $initialChunkRadius,
    ) {
        $this->logger = $logs->logger('Cobblestone.Server');
        $this->events = new EventBus();
        $this->commands = new CommandRegistry();
        $this->scheduler = new Scheduler();
        $this->plugins = new PluginManager(
            $this->events,
            $this->commands,
            $this->scheduler,
            $this->logs,
        );
        $this->bootstrap = new SessionBootstrap($this->sessions, $this->world, $initialChunkRadius);
        $this->gameplay = new SessionGameplay($this->sessions, $this->world);
        $this->worldMaintenance = new WorldMaintenance($this->sessions, $this->world, $this->logger);

        $this->state = ServerState::Running;
        $this->events->dispatch(new ServerStarted());
        $this->logger->info(
            'Server lifecycle started',
            [
                'world' => $world->name(),
                'generator' => $world->generator()->name(),
                'initial_chunk_radius' => $initialChunkRadius,
            ],
        );
    }

    /** @param Closure(Packet): void|null $packetHandler */
    public static function start(
        string $bind,
        int $maxConnections,
        string $serverName,
        ?Closure $packetHandler = null,
        ?LoggerFactory $logs = null,
        ?World $world = null,
        int $initialChunkRadius = 2,
    ): self {
        $logs ??= LoggerFactory::console(getenv('COBBLESTONE_LOG_LEVEL') ?: 'INFO');
        $world ??= WorldFactory::flat();
        $sessions = Runtime::start($bind, $maxConnections, $serverName);

        try {
            return new self(
                $sessions,
                $logs,
                $packetHandler,
                $serverName,
                $world,
                $initialChunkRadius,
            );
        } catch (Throwable $error) {
            try {
                $sessions->stop();
            } catch (Throwable) {
            }
            throw $error;
        }
    }

    public function events(): EventBus { return $this->events; }
    public function commands(): CommandRegistry { return $this->commands; }
    public function scheduler(): Scheduler { return $this->scheduler; }
    public function plugins(): PluginManager { return $this->plugins; }
    public function logger(): LoggerInterface { return $this->logger; }
    public function state(): ServerState { return $this->state; }
    public function world(): World { return $this->world; }

    /**
     * Suspends the current scheduler-managed Fiber until this chunk is resident.
     *
     * Persistent storage is allowed to generate only after the native load path reports a durable
     * miss. Already-resident and non-persistent chunks complete synchronously. The caller owns the
     * returned residency pin and must release it when the gameplay operation is finished.
     */
    public function awaitResidentChunk(ChunkPos $position): ResidentChunkHandle
    {
        $this->assertRunning();

        while (true) {
            try {
                return $this->world->residentChunk($position, true)
                    ?? throw new LogicException(
                        "chunk {$position->x}:{$position->z} did not become resident",
                    );
            } catch (ChunkLoadPending) {
                Scheduler::sleep(1);
            }
        }
    }

    public function isStopRequested(): bool
    {
        return $this->stopReason !== null || $this->state !== ServerState::Running;
    }

    public function requestStop(string $reason = 'requested'): void
    {
        if ($this->state !== ServerState::Running || $this->stopReason !== null) {
            return;
        }

        $this->stopReason = $reason;
        $this->logger->info('Shutdown requested', ['reason' => $reason]);
    }

    public function tick(int $nativeEventBudget = 256): void
    {
        $this->assertRunning();
        if ($nativeEventBudget <= 0) {
            throw new LogicException('native event budget must be positive');
        }

        $this->scheduler->tick();

        for ($processed = 0; $processed < $nativeEventBudget; ++$processed) {
            $event = $this->sessions->poll();
            if ($event === null) {
                break;
            }

            if ($event instanceof Connected) {
                $this->bootstrap->connected($event->sessionId);
                $this->logger->info(
                    'Session connected',
                    ['session' => $event->sessionId, 'peer' => $event->peer],
                );
                $this->events->dispatch(new SessionConnected($event->sessionId, $event->peer));
                continue;
            }
            if ($event instanceof Disconnected) {
                $this->bootstrap->disconnected($event->sessionId);
                $this->gameplay->disconnected($event->sessionId);
                $this->logger->info(
                    'Session disconnected',
                    ['session' => $event->sessionId, 'reason' => $event->reason],
                );
                $this->events->dispatch(new SessionDisconnected($event->sessionId, $event->reason));
                continue;
            }
            if ($event instanceof Packet) {
                try {
                    $update = $this->bootstrap->handle($event);
                } catch (Throwable $error) {
                    $this->logger->error(
                        'Session bootstrap failed',
                        [
                            'session' => $event->sessionId,
                            'packet' => $event->packetId,
                            'exception' => $error,
                        ],
                    );
                    try {
                        $this->sessions->disconnect($event->sessionId);
                    } catch (Throwable $disconnectError) {
                        $this->logger->warning(
                            'Session disconnect after bootstrap failure failed',
                            [
                                'session' => $event->sessionId,
                                'exception' => $disconnectError,
                            ],
                        );
                    }
                    continue;
                }

                if ($update->kind === BootstrapUpdate::LOGIN_ACCEPTED) {
                    $this->logger->info(
                        'Session login accepted',
                        ['session' => $event->sessionId, 'protocol' => 84],
                    );
                    $this->events->dispatch(new SessionLoginAccepted($event->sessionId));
                    continue;
                }
                if ($update->kind === BootstrapUpdate::CHUNKS_LOADING) {
                    continue;
                }
                if ($update->kind === BootstrapUpdate::SPAWNED) {
                    $this->dispatchSpawned($event->sessionId, $update);
                    continue;
                }

                try {
                    $this->gameplay->handle($event);
                } catch (Throwable $error) {
                    $this->logger->error(
                        'Session gameplay packet failed',
                        [
                            'session' => $event->sessionId,
                            'packet' => $event->packetId,
                            'exception' => $error,
                        ],
                    );
                    try {
                        $this->sessions->disconnect($event->sessionId);
                    } catch (Throwable $disconnectError) {
                        $this->logger->warning(
                            'Session disconnect after gameplay failure failed',
                            [
                                'session' => $event->sessionId,
                                'exception' => $disconnectError,
                            ],
                        );
                    }
                    continue;
                }

                if ($this->packetHandler !== null) {
                    ($this->packetHandler)($event);
                }
            }
        }

        $this->worldMaintenance->tickStorage();

        foreach ($this->bootstrap->tick() as $completion) {
            $this->dispatchSpawned($completion['sessionId'], $completion['update']);
        }

        $this->gameplay->tick();
        $this->worldMaintenance->flushWorldChanges();
    }

    public function stop(): void
    {
        if ($this->state === ServerState::Stopped || $this->state === ServerState::Stopping) {
            return;
        }

        $this->state = ServerState::Stopping;
        $reason = $this->stopReason ?? 'shutdown';
        $this->logger->info('Stopping server', ['reason' => $reason]);

        $failure = null;

        foreach ([
            'stopping-event' => fn () => $this->events->dispatch(new ServerStopping()),
            'gameplay' => fn () => $this->gameplay->stop(),
            'sessions' => fn () => $this->sessions->stop(),
            'plugins' => fn () => $this->plugins->shutdown(),
            'scheduler' => fn () => $this->scheduler->shutdown(),
            'world-storage' => function (): void {
                $nativeStore = $this->world->nativeStore();
                if ($nativeStore !== null && $nativeStore->hasStorage()) {
                    $nativeStore->flushStorage();
                }
            },
        ] as $phase => $shutdown) {
            try {
                $shutdown();
            } catch (Throwable $error) {
                $failure ??= $error;
                $this->logger->error(
                    'Shutdown phase failed',
                    ['phase' => $phase, 'exception' => $error],
                );
            }
        }

        $this->state = ServerState::Stopped;
        $this->logger->info('Server stopped', ['reason' => $reason]);

        if ($failure !== null) {
            throw $failure;
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

    private function assertRunning(): void
    {
        if ($this->state !== ServerState::Running || !$this->sessions->isRunning()) {
            throw new LogicException('Cobblestone server is not running');
        }
    }
}
