<?php

declare(strict_types=1);

namespace Cobblestone\Kernel;

use Closure;
use Cobblestone\Internal\NativeSessionConnected;
use Cobblestone\Internal\NativeSessionDisconnected;
use Cobblestone\Internal\NativeSessionPacket;
use Cobblestone\Internal\NativeSessionRuntime;
use Cobblestone\Internal\Protocol84Bootstrap;
use Cobblestone\Internal\Protocol84BootstrapResult;
use Cobblestone\Plugin\PluginContext;
use Cobblestone\Plugin\PluginManager;
use LogicException;
use Throwable;

final readonly class ServerStarted
{
}

final readonly class ServerStopping
{
}

final readonly class SessionConnected
{
    public function __construct(
        public int $sessionId,
        public string $peer,
    ) {
    }
}

final readonly class SessionDisconnected
{
    public function __construct(
        public int $sessionId,
        public string $reason,
    ) {
    }
}

final readonly class SessionLoginAccepted
{
    public function __construct(
        public int $sessionId,
    ) {
    }
}

final readonly class SessionSpawned
{
    public function __construct(
        public int $sessionId,
        public int $requestedRadius,
        public int $probeRadius,
    ) {
    }
}

/**
 * Single-owner PHP server kernel for the C007 foundation.
 *
 * Raw protocol packets stay on an internal handler and are never published on the plugin EventBus.
 */
final class ServerKernel
{
    private readonly EventBus $events;
    private readonly CommandRegistry $commands;
    private readonly Scheduler $scheduler;
    private readonly PluginManager $plugins;
    private readonly Protocol84Bootstrap $bootstrap;
    private bool $running = true;

    /**
     * @param Closure(NativeSessionPacket): void|null $packetHandler
     */
    private function __construct(
        private readonly NativeSessionRuntime $sessions,
        private ?Closure $packetHandler,
    ) {
        $this->events = new EventBus();
        $this->commands = new CommandRegistry();
        $this->scheduler = new Scheduler();
        $this->plugins = new PluginManager(
            new PluginContext($this->events, $this->commands, $this->scheduler),
        );
        $this->bootstrap = new Protocol84Bootstrap($this->sessions);
        $this->events->dispatch(new ServerStarted());
    }

    /**
     * @param Closure(NativeSessionPacket): void|null $packetHandler
     */
    public static function start(
        string $bind,
        int $maxConnections,
        string $serverName,
        ?Closure $packetHandler = null,
    ): self {
        return new self(
            NativeSessionRuntime::start($bind, $maxConnections, $serverName),
            $packetHandler,
        );
    }

    public function events(): EventBus
    {
        return $this->events;
    }

    public function commands(): CommandRegistry
    {
        return $this->commands;
    }

    public function scheduler(): Scheduler
    {
        return $this->scheduler;
    }

    public function plugins(): PluginManager
    {
        return $this->plugins;
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

            if ($event instanceof NativeSessionConnected) {
                $this->bootstrap->connected($event->sessionId);
                $this->events->dispatch(new SessionConnected($event->sessionId, $event->peer));
                continue;
            }
            if ($event instanceof NativeSessionDisconnected) {
                $this->bootstrap->disconnected($event->sessionId);
                $this->events->dispatch(new SessionDisconnected($event->sessionId, $event->reason));
                continue;
            }
            if ($event instanceof NativeSessionPacket) {
                try {
                    $result = $this->bootstrap->handle($event);
                } catch (Throwable $error) {
                    fprintf(
                        STDERR,
                        "cobblestone-kernel: bootstrap-error session=%d type=%s message=%s\\n",
                        $event->sessionId,
                        get_class($error),
                        $error->getMessage(),
                    );

                    try {
                        $this->sessions->disconnect($event->sessionId);
                    } catch (Throwable) {
                    }
                    continue;
                }

                if ($result->kind === Protocol84BootstrapResult::LOGIN_ACCEPTED) {
                    $this->events->dispatch(new SessionLoginAccepted($event->sessionId));
                    continue;
                }
                if ($result->kind === Protocol84BootstrapResult::SPAWNED) {
                    $this->events->dispatch(
                        new SessionSpawned(
                            $event->sessionId,
                            $result->requestedRadius ?? 0,
                            2,
                        ),
                    );
                    continue;
                }

                if ($this->packetHandler !== null) {
                    ($this->packetHandler)($event);
                }
            }
        }
    }

    public function stop(): void
    {
        if (!$this->running) {
            return;
        }

        $failure = null;
        try {
            $this->events->dispatch(new ServerStopping());
        } catch (Throwable $error) {
            $failure = $error;
        }

        try {
            $this->sessions->stop();
        } catch (Throwable $error) {
            $failure ??= $error;
        }

        $this->scheduler->shutdown();

        try {
            $this->plugins->shutdown();
        } catch (Throwable $error) {
            $failure ??= $error;
        }

        $this->running = false;

        if ($failure !== null) {
            throw $failure;
        }
    }

    private function assertRunning(): void
    {
        if (!$this->running || !$this->sessions->isRunning()) {
            throw new LogicException('Cobblestone server kernel is not running');
        }
    }
}
