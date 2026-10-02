<?php

declare(strict_types=1);

namespace Cobblestone\Plugin;

use Closure;
use Cobblestone\Command\CommandBinding;
use Cobblestone\Command\CommandRegistry;
use Cobblestone\Command\Literal;
use Cobblestone\Event\EventBus;
use Cobblestone\Event\Subscription;
use Cobblestone\Task\Scheduler;
use Cobblestone\Task\TaskHandle;
use LogicException;
use Psr\Log\LoggerInterface;
use Throwable;

final class PluginScope
{
    /** @var list<Closure(): void> */
    private array $cleanup = [];

    private bool $closed = false;

    public function __construct(
        private readonly EventBus $events,
        private readonly CommandRegistry $commands,
        private readonly Scheduler $scheduler,
        private readonly LoggerInterface $logger,
    ) {
    }

    public function logger(): LoggerInterface
    {
        return $this->logger;
    }

    /** @param class-string $event */
    public function on(string $event, Closure $listener): Subscription
    {
        $this->assertOpen();
        $subscription = $this->events->listen($event, $listener);
        $this->own(static function () use ($subscription): void {
            $subscription->cancel();
        });

        return $subscription;
    }

    public function command(Literal $root): CommandBinding
    {
        $this->assertOpen();
        $binding = $this->commands->register($root);
        $this->own(static function () use ($binding): void {
            $binding->cancel();
        });

        return $binding;
    }

    public function after(int $delayTicks, Closure $task): TaskHandle
    {
        $this->assertOpen();

        return $this->ownTask($this->scheduler->schedule($delayTicks, $task));
    }

    public function every(int $intervalTicks, Closure $task): TaskHandle
    {
        $this->assertOpen();

        return $this->ownTask($this->scheduler->repeat($intervalTicks, $task));
    }

    public function task(Closure $entry): TaskHandle
    {
        $this->assertOpen();

        return $this->ownTask($this->scheduler->spawn($entry));
    }

    public function cleanup(Closure $cleanup): void
    {
        $this->assertOpen();
        $this->own($cleanup);
    }

    /** @internal PluginManager owns scope closure. */
    public function close(): void
    {
        if ($this->closed) {
            return;
        }
        $this->closed = true;

        $failure = null;
        foreach (array_reverse($this->cleanup) as $cleanup) {
            try {
                $cleanup();
            } catch (Throwable $error) {
                $failure ??= $error;
            }
        }
        $this->cleanup = [];

        if ($failure !== null) {
            throw $failure;
        }
    }

    private function ownTask(TaskHandle $handle): TaskHandle
    {
        $this->own(static function () use ($handle): void {
            $handle->cancel();
        });

        return $handle;
    }

    /** @param Closure(): void $cleanup */
    private function own(Closure $cleanup): void
    {
        $this->cleanup[] = $cleanup;
    }

    private function assertOpen(): void
    {
        if ($this->closed) {
            throw new LogicException('plugin scope is closed');
        }
    }
}
