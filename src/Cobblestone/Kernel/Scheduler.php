<?php

declare(strict_types=1);

namespace Cobblestone\Kernel;

use Closure;
use Fiber;
use InvalidArgumentException;
use LogicException;

/** @internal */
final readonly class NativeTaskAwait
{
    public function __construct(public int $taskId)
    {
        if ($taskId <= 0) {
            throw new InvalidArgumentException('native task id must be positive');
        }
    }
}

/** @internal */
final readonly class TickSleep
{
    public function __construct(public int $ticks)
    {
        if ($ticks <= 0) {
            throw new InvalidArgumentException('sleep ticks must be positive');
        }
    }
}

/**
 * Single-owner tick scheduler with Fiber integration.
 *
 * Fibers are resumed only from tick() on the owning PHP runtime. Native workers publish completion
 * state through the existing bounded native completion mechanism and never invoke Zend directly.
 */
final class Scheduler
{
    private int $tick = 0;
    private int $nextTaskId = 1;
    private int $nextFiberId = 1;

    /**
     * @var array<int, array{due: int, interval: ?int, task: Closure(): void}>
     */
    private array $tasks = [];

    /** @var array<int, Fiber<mixed, mixed, mixed, mixed>> */
    private array $fibers = [];

    /**
     * @var array<int, NativeTaskAwait|array{wake: int}>
     */
    private array $waiting = [];

    /**
     * Schedules a one-shot task after the requested number of future ticks.
     *
     * A zero-delay task cannot run in the already-active tick, so zero and one both become due on
     * the next owner-runtime tick.
     */
    public function schedule(int $delayTicks, Closure $task): int
    {
        if ($delayTicks < 0) {
            throw new InvalidArgumentException('delay ticks must be non-negative');
        }

        $id = $this->nextTaskId++;
        $this->tasks[$id] = [
            'due' => $this->tick + max(1, $delayTicks),
            'interval' => null,
            'task' => $task,
        ];

        return $id;
    }

    public function repeat(int $intervalTicks, Closure $task): int
    {
        if ($intervalTicks <= 0) {
            throw new InvalidArgumentException('repeat interval must be positive');
        }

        $id = $this->nextTaskId++;
        $this->tasks[$id] = [
            'due' => $this->tick + $intervalTicks,
            'interval' => $intervalTicks,
            'task' => $task,
        ];

        return $id;
    }

    public function cancel(int $taskId): bool
    {
        if (!isset($this->tasks[$taskId])) {
            return false;
        }

        unset($this->tasks[$taskId]);

        return true;
    }

    /**
     * Starts a Fiber immediately on the owner runtime and tracks any supported suspension.
     */
    public function spawn(Closure $entry): int
    {
        $id = $this->nextFiberId++;
        $fiber = new Fiber($entry);
        $this->fibers[$id] = $fiber;
        $yielded = $fiber->start();
        $this->captureFiberState($id, $yielded);

        return $id;
    }

    public static function awaitNative(int $taskId): mixed
    {
        return Fiber::suspend(new NativeTaskAwait($taskId));
    }

    public static function sleep(int $ticks): void
    {
        Fiber::suspend(new TickSleep($ticks));
    }

    public function tick(): void
    {
        ++$this->tick;

        foreach (array_keys($this->tasks) as $taskId) {
            $scheduled = $this->tasks[$taskId] ?? null;
            if ($scheduled === null || $scheduled['due'] > $this->tick) {
                continue;
            }

            ($scheduled['task'])();

            if (!isset($this->tasks[$taskId])) {
                continue;
            }
            if ($scheduled['interval'] === null) {
                unset($this->tasks[$taskId]);
            } else {
                $this->tasks[$taskId]['due'] = $this->tick + $scheduled['interval'];
            }
        }

        foreach (array_keys($this->waiting) as $fiberId) {
            $wait = $this->waiting[$fiberId] ?? null;
            $fiber = $this->fibers[$fiberId] ?? null;
            if ($wait === null || $fiber === null) {
                continue;
            }

            if ($wait instanceof NativeTaskAwait) {
                if (!cobblestone_core_async_ready($wait->taskId)) {
                    continue;
                }
                $yielded = $fiber->resume(cobblestone_core_async_take($wait->taskId));
            } else {
                if ($wait['wake'] > $this->tick) {
                    continue;
                }
                $yielded = $fiber->resume();
            }

            $this->captureFiberState($fiberId, $yielded);
        }
    }

    public function shutdown(): void
    {
        $this->tasks = [];
        $this->waiting = [];
        $this->fibers = [];
    }

    private function captureFiberState(int $fiberId, mixed $yielded): void
    {
        $fiber = $this->fibers[$fiberId] ?? null;
        if ($fiber === null) {
            return;
        }
        if ($fiber->isTerminated()) {
            unset($this->fibers[$fiberId], $this->waiting[$fiberId]);

            return;
        }

        $this->waiting[$fiberId] = match (true) {
            $yielded instanceof NativeTaskAwait => $yielded,
            $yielded instanceof TickSleep => ['wake' => $this->tick + $yielded->ticks],
            default => throw new LogicException(
                'Cobblestone Fiber suspended without Scheduler::awaitNative() or Scheduler::sleep()',
            ),
        };
    }
}
