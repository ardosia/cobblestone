<?php

declare(strict_types=1);

namespace Cobblestone\Server;

use Closure;
use Cobblestone\Command\CommandBinding;
use Cobblestone\Command\CommandRegistry;
use Cobblestone\Command\Literal;
use Cobblestone\Event\EventBus;
use Cobblestone\Event\Subscription;
use Cobblestone\Log\LoggerFactory;
use Cobblestone\Native\Session\Packet;
use Cobblestone\Plugin\Plugin;
use Cobblestone\Plugin\PluginManager;
use Cobblestone\Server\Event\ServerStarted;
use Cobblestone\Server\Event\ServerStopping;
use Cobblestone\Server\Internal\Runtime as ServerRuntime;
use Cobblestone\Task\Scheduler;
use Cobblestone\Task\TaskHandle;
use Cobblestone\World\ChunkLease;
use Cobblestone\World\ChunkLoadPending;
use Cobblestone\World\ChunkPos;
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

    private ?ServerRuntime $runtime = null;
    private ServerState $state = ServerState::Created;
    private ?string $stopReason = null;

    /** @param Closure(Packet): void|null $packetHandler */
    private function __construct(
        private readonly ServerConfig $config,
        private readonly LoggerFactory $logs,
        private readonly World $world,
        private readonly ?Closure $packetHandler,
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
    }

    /** @param Closure(Packet): void|null $packetHandler */
    public static function create(
        ServerConfig $config,
        ?World $world = null,
        ?LoggerFactory $logs = null,
        ?Closure $packetHandler = null,
    ): self {
        $logs ??= LoggerFactory::console(getenv('COBBLESTONE_LOG_LEVEL') ?: 'INFO');
        $world ??= WorldFactory::flat();

        return new self(
            $config,
            $logs,
            $world,
            $packetHandler,
        );
    }

    public function config(): ServerConfig
    {
        return $this->config;
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
        $runtime = null;

        try {
            $runtime = ServerRuntime::start(
                $this->config,
                $this->world,
                $this->events,
                $this->scheduler,
                $this->logger,
                $this->packetHandler,
            );
            $this->runtime = $runtime;
            $this->state = ServerState::Running;

            $this->events->dispatch(new ServerStarted());
            $this->logger->info(
                'Server lifecycle started',
                [
                    'world' => $this->world->name(),
                    'generator' => $this->world->generator()->name(),
                    'initial_chunk_radius' => $this->config->initialChunkRadius,
                ],
            );
        } catch (Throwable $error) {
            $runtime?->abortStart();
            $this->runtime = null;
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

    public function command(Literal $root): CommandBinding
    {
        return $this->commands->register($root);
    }

    public function executeCommand(string $input, mixed $source = null): mixed
    {
        return $this->commands->execute($input, $source);
    }

    /** @return list<string> */
    public function suggestCommand(string $input, mixed $source = null): array
    {
        return $this->commands->suggest($input, $source);
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
    public function awaitResidentChunk(ChunkPos $position): ChunkLease
    {
        $this->assertRunning();

        while (true) {
            try {
                return $this->world->pinChunk($position, true)
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
        $this->serverRuntime()->tick($nativeEventBudget);
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
        if ($this->runtime !== null) {
            $phases['gameplay'] = fn () => $this->runtime?->stopGameplay();
            $phases['sessions'] = fn () => $this->runtime?->stopSessions();
        }
        $phases['plugins'] = fn () => $this->plugins->shutdown();
        $phases['scheduler'] = fn () => $this->scheduler->shutdown();
        $phases['world-storage'] = function (): void {
            $native = $this->world->nativeStore();
            if ($native !== null && $native->hasStorage()) {
                $native->flushStorage();
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

        $this->runtime = null;
        $this->state = ServerState::Stopped;
        $this->logger->info('Server stopped', ['reason' => $reason]);

        if ($failure !== null) {
            throw $failure;
        }
    }

    private function assertRunning(): void
    {
        if (
            $this->state !== ServerState::Running
            || $this->runtime === null
            || !$this->runtime->running()
        ) {
            throw new LogicException('Cobblestone server is not running');
        }
    }

    private function serverRuntime(): ServerRuntime
    {
        return $this->runtime ?? throw new LogicException('server runtime is unavailable');
    }
}
