<?php

declare(strict_types=1);

namespace Cobblestone\Server;

use Closure;
use Cobblestone\Command\CommandRegistry;
use Cobblestone\Event\EventBus;
use Cobblestone\Event\Subscription;
use Cobblestone\Log\LoggerFactory;
use Cobblestone\Native\Session\Connected;
use Cobblestone\Native\Session\Disconnected;
use Cobblestone\Native\Session\Packet;
use Cobblestone\Native\Session\Runtime;
use Cobblestone\Plugin\Plugin;
use Cobblestone\Plugin\PluginManager;
use Cobblestone\Server\Event\ServerStarted;
use Cobblestone\Server\Event\ServerStopping;
use Cobblestone\Session\BootstrapUpdate;
use Cobblestone\Session\Event\SessionConnected;
use Cobblestone\Session\Event\SessionDisconnected;
use Cobblestone\Session\Event\SessionLoginAccepted;
use Cobblestone\Session\Event\SessionSpawned;
use Cobblestone\Session\SessionBootstrap;
use Cobblestone\Session\SessionGameplay;
use Cobblestone\Task\Scheduler;
use Cobblestone\Task\TaskHandle;
use Cobblestone\World\ChunkLoadPending;
use Cobblestone\World\ChunkPos;
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
    private readonly LoggerInterface $logger;

    private ?Runtime $sessions = null;
    private ?SessionBootstrap $bootstrap = null;
    private ?SessionGameplay $gameplay = null;
    private ?WorldMaintenance $worldMaintenance = null;

    private ServerState $state = ServerState::Created;
    private ?string $stopReason = null;

    /** @param Closure(Packet): void|null $packetHandler */
    private function __construct(
        private readonly string $bind,
        private readonly int $maxConnections,
        private readonly LoggerFactory $logs,
        private ?Closure $packetHandler,
        private readonly string $serverName,
        private readonly World $world,
        private readonly int $initialChunkRadius,
    ) {
        if ($maxConnections <= 0) {
            throw new \InvalidArgumentException('max connections must be positive');
        }
        if ($initialChunkRadius < 0) {
            throw new \InvalidArgumentException('initial chunk radius must be non-negative');
        }

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
    }

    /** @param Closure(Packet): void|null $packetHandler */
    public static function create(
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

        return new self(
            $bind,
            $maxConnections,
            $logs,
            $packetHandler,
            $serverName,
            $world,
            $initialChunkRadius,
        );
    }

    public function start(): void
    {
        if ($this->state === ServerState::Running) {
            return;
        }
        if ($this->state !== ServerState::Created) {
            throw new LogicException("cannot start server from {$this->state->value} state");
        }
        if ($this->stopReason !== null) {
            throw new LogicException('cannot start server after shutdown was requested');
        }

        $this->state = ServerState::Starting;
        $sessions = null;

        try {
            $sessions = Runtime::start($this->bind, $this->maxConnections, $this->serverName);
            $this->sessions = $sessions;
            $this->bootstrap = new SessionBootstrap($sessions, $this->world, $this->initialChunkRadius);
            $this->gameplay = new SessionGameplay($sessions, $this->world, $this->initialChunkRadius);
            $this->worldMaintenance = new WorldMaintenance($sessions, $this->world, $this->logger);

            $this->state = ServerState::Running;
            $this->events->dispatch(new ServerStarted());
            $this->logger->info(
                'Server lifecycle started',
                [
                    'world' => $this->world->name(),
                    'generator' => $this->world->generator()->name(),
                    'initial_chunk_radius' => $this->initialChunkRadius,
                ],
            );
        } catch (Throwable $error) {
            if ($sessions !== null) {
                try {
                    $sessions->stop();
                } catch (Throwable) {
                }
            }
            $this->sessions = null;
            $this->bootstrap = null;
            $this->gameplay = null;
            $this->worldMaintenance = null;
            $this->state = ServerState::Created;
            throw $error;
        }
    }

    /** @param class-string $event */
    public function on(string $event, Closure $listener): Subscription
    {
        return $this->events->listen($event, $listener);
    }

    /** @param class-string<Plugin> $class */
    public function loadPlugin(string $file, string $class): Plugin
    {
        return $this->plugins->load($file, $class);
    }

    /** @param class-string<Plugin> $class */
    public function unloadPlugin(string $class): bool
    {
        return $this->plugins->unload($class);
    }

    public function after(int $delayTicks, Closure $task): TaskHandle
    {
        return $this->scheduler->schedule($delayTicks, $task);
    }

    public function every(int $intervalTicks, Closure $task): TaskHandle
    {
        return $this->scheduler->repeat($intervalTicks, $task);
    }

    public function task(Closure $entry): TaskHandle
    {
        return $this->scheduler->spawn($entry);
    }

    /**
     * @internal Temporary execution bridge until the typed command tree owns parsing/dispatch.
     * @param list<string> $arguments
     */
    public function executeCommand(string $name, array $arguments = []): mixed
    {
        return $this->commands->execute($name, $arguments);
    }

    public function logger(): LoggerInterface
    {
        return $this->logger;
    }

    public function state(): ServerState
    {
        return $this->state;
    }

    public function world(): World
    {
        return $this->world;
    }

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
        return $this->stopReason !== null
            || $this->state === ServerState::Stopping
            || $this->state === ServerState::Stopped;
    }

    public function stop(string $reason = 'requested'): void
    {
        if (
            $this->state === ServerState::Stopped
            || $this->state === ServerState::Stopping
            || $this->stopReason !== null
        ) {
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

        $sessions = $this->sessionRuntime();
        $bootstrap = $this->sessionBootstrap();
        $gameplay = $this->sessionGameplay();
        $worldMaintenance = $this->worldMaintenance();

        $this->scheduler->tick();

        for ($processed = 0; $processed < $nativeEventBudget; ++$processed) {
            $event = $sessions->poll();
            if ($event === null) {
                break;
            }

            if ($event instanceof Connected) {
                $bootstrap->connected($event->sessionId);
                $this->logger->info(
                    'Session connected',
                    ['session' => $event->sessionId, 'peer' => $event->peer],
                );
                $this->events->dispatch(new SessionConnected($event->sessionId, $event->peer));
                continue;
            }
            if ($event instanceof Disconnected) {
                $bootstrap->disconnected($event->sessionId);
                $gameplay->disconnected($event->sessionId);
                $this->logger->info(
                    'Session disconnected',
                    ['session' => $event->sessionId, 'reason' => $event->reason],
                );
                $this->events->dispatch(new SessionDisconnected($event->sessionId, $event->reason));
                continue;
            }
            if ($event instanceof Packet) {
                try {
                    $update = $bootstrap->handle($event);
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
                        $sessions->disconnect($event->sessionId);
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
                    $gameplay->handle($event);
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
                        $sessions->disconnect($event->sessionId);
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

        $worldMaintenance->tickStorage();

        foreach ($bootstrap->tick() as $completion) {
            $this->dispatchSpawned($completion['sessionId'], $completion['update']);
        }

        $gameplay->tick();
        $worldMaintenance->flushWorldChanges();
    }

    /** @internal ServerRunner owns terminal shutdown. */
    public function shutdown(): void
    {
        if ($this->state === ServerState::Stopped || $this->state === ServerState::Stopping) {
            return;
        }

        $previous = $this->state;
        $this->state = ServerState::Stopping;
        $reason = $this->stopReason ?? 'shutdown';
        $this->logger->info('Stopping server', ['reason' => $reason]);

        $failure = null;
        $phases = [];
        if ($previous === ServerState::Running) {
            $phases['stopping-event'] = fn () => $this->events->dispatch(new ServerStopping());
        }
        if ($this->gameplay !== null) {
            $phases['gameplay'] = fn () => $this->gameplay?->stop();
        }
        if ($this->sessions !== null) {
            $phases['sessions'] = fn () => $this->sessions?->stop();
        }
        $phases['plugins'] = fn () => $this->plugins->shutdown();
        $phases['scheduler'] = fn () => $this->scheduler->shutdown();
        $phases['world-storage'] = function (): void {
            $nativeStore = $this->world->nativeStore();
            if ($nativeStore !== null && $nativeStore->hasStorage()) {
                $nativeStore->flushStorage();
            }
        };

        foreach ($phases as $phase => $shutdown) {
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

        $this->sessions = null;
        $this->bootstrap = null;
        $this->gameplay = null;
        $this->worldMaintenance = null;
        $this->state = ServerState::Stopped;
        $this->logger->info('Server stopped', ['reason' => $reason]);

        if ($failure !== null) {
            throw $failure;
        }
    }

    private function dispatchSpawned(int $sessionId, BootstrapUpdate $update): void
    {
        $this->sessionGameplay()->spawned($sessionId);
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
        if (
            $this->state !== ServerState::Running
            || $this->sessions === null
            || !$this->sessions->isRunning()
        ) {
            throw new LogicException('Cobblestone server is not running');
        }
    }

    private function sessionRuntime(): Runtime
    {
        return $this->sessions ?? throw new LogicException('session runtime is unavailable');
    }

    private function sessionBootstrap(): SessionBootstrap
    {
        return $this->bootstrap ?? throw new LogicException('session bootstrap is unavailable');
    }

    private function sessionGameplay(): SessionGameplay
    {
        return $this->gameplay ?? throw new LogicException('session gameplay is unavailable');
    }

    private function worldMaintenance(): WorldMaintenance
    {
        return $this->worldMaintenance ?? throw new LogicException('world maintenance is unavailable');
    }
}
