<?php

declare(strict_types=1);

namespace Cobblestone\Server;

use Closure;
use Cobblestone\Command\CommandRegistry;
use Cobblestone\Event\EventBus;
use Cobblestone\Native\Session\Connected;
use Cobblestone\Native\Session\Disconnected;
use Cobblestone\Native\Session\Packet;
use Cobblestone\Native\Session\Runtime;
use Cobblestone\Plugin\PluginContext;
use Cobblestone\Plugin\PluginManager;
use Cobblestone\Server\Event\ServerStarted;
use Cobblestone\Server\Event\ServerStopping;
use Cobblestone\Session\Event\SessionConnected;
use Cobblestone\Session\Event\SessionDisconnected;
use Cobblestone\Session\Event\SessionLoginAccepted;
use Cobblestone\Session\Event\SessionSpawned;
use Cobblestone\Session\JoinFlow;
use Cobblestone\Session\JoinResult;
use Cobblestone\Task\Scheduler;
use LogicException;
use Throwable;

final class Server
{
    private readonly EventBus $events;
    private readonly CommandRegistry $commands;
    private readonly Scheduler $scheduler;
    private readonly PluginManager $plugins;
    private readonly JoinFlow $join;
    private bool $running = true;

    /** @param Closure(Packet): void|null $packetHandler */
    private function __construct(
        private readonly Runtime $sessions,
        private ?Closure $packetHandler,
    ) {
        $this->events = new EventBus();
        $this->commands = new CommandRegistry();
        $this->scheduler = new Scheduler();
        $this->plugins = new PluginManager(
            new PluginContext($this->events, $this->commands, $this->scheduler),
        );
        $this->join = new JoinFlow($this->sessions);
        $this->events->dispatch(new ServerStarted());
    }

    /** @param Closure(Packet): void|null $packetHandler */
    public static function start(
        string $bind,
        int $maxConnections,
        string $serverName,
        ?Closure $packetHandler = null,
    ): self {
        return new self(Runtime::start($bind, $maxConnections, $serverName), $packetHandler);
    }

    public function events(): EventBus { return $this->events; }
    public function commands(): CommandRegistry { return $this->commands; }
    public function scheduler(): Scheduler { return $this->scheduler; }
    public function plugins(): PluginManager { return $this->plugins; }

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
                $this->join->connected($event->sessionId);
                $this->events->dispatch(new SessionConnected($event->sessionId, $event->peer));
                continue;
            }
            if ($event instanceof Disconnected) {
                $this->join->disconnected($event->sessionId);
                $this->events->dispatch(new SessionDisconnected($event->sessionId, $event->reason));
                continue;
            }
            if ($event instanceof Packet) {
                try {
                    $result = $this->join->handle($event);
                } catch (Throwable $error) {
                    fprintf(
                        STDERR,
                        "cobblestone: join-error session=%d type=%s message=%s\n",
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

                if ($result->kind === JoinResult::LOGIN_ACCEPTED) {
                    $this->events->dispatch(new SessionLoginAccepted($event->sessionId));
                    continue;
                }
                if ($result->kind === JoinResult::SPAWNED) {
                    $this->events->dispatch(
                        new SessionSpawned($event->sessionId, $result->requestedRadius ?? 0, 2),
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
            throw new LogicException('Cobblestone server is not running');
        }
    }
}
