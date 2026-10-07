<?php

declare(strict_types=1);

namespace Cobblestone\Task;

use Closure;
use Fiber;
use Cobblestone\Task\Internal\DueQueue;
use Cobblestone\Task\Internal\Sleep;
use InvalidArgumentException;
use LogicException;
use Throwable;

/**
 * Single-owner tick scheduler with Fiber integration.
 *
 * Scheduled work and sleeping fibers live in due-time min-heaps, so dormant entries do not add
 * fixed work to every tick. Fibers are started and resumed only from tick() on the owning PHP
 * runtime.
 */
final class Scheduler
{
    private int $tick = 0;
    private int $nextTaskId = 1;
    private int $nextFiberId = 1;
    private int $staleTaskEntries = 0;
    private readonly DueQueue $taskQueue;
    private readonly DueQueue $sleepQueue;

    /** @var array<int, array{due: int, interval: ?int, task: Closure(): void}> */
    private array $tasks = [];

    /** @var array<int, Fiber<mixed, mixed, mixed, mixed>> */
    private array $fibers = [];

    /** @var array<int, true> */
    private array $pendingFibers = [];

    /** @var array<int, int> fiber id => wake tick */
    private array $sleeping = [];

    public function __construct()
    {
        $this->taskQueue = new DueQueue();
        $this->sleepQueue = new DueQueue();
    }

    public function schedule(int $delayTicks, Closure $task): TaskHandle
    {
        if ($delayTicks < 0) {
            throw new InvalidArgumentException('delay ticks must be non-negative');
        }

        $id = $this->nextTaskId++;
        $due = $this->tick + max(1, $delayTicks);
        $this->tasks[$id] = [
            'due' => $due,
            'interval' => null,
            'task' => $task,
        ];
        $this->taskQueue->push($due, $id);

        return $this->scheduledHandle($id);
    }

    public function repeat(int $intervalTicks, Closure $task): TaskHandle
    {
        if ($intervalTicks <= 0) {
            throw new InvalidArgumentException('repeat interval must be positive');
        }

        $id = $this->nextTaskId++;
        $due = $this->tick + $intervalTicks;
        $this->tasks[$id] = [
            'due' => $due,
            'interval' => $intervalTicks,
            'task' => $task,
        ];
        $this->taskQueue->push($due, $id);

        return $this->scheduledHandle($id);
    }

    public function spawn(Closure $entry): TaskHandle
    {
        $id = $this->nextFiberId++;
        $this->fibers[$id] = new Fiber($entry);
        $this->pendingFibers[$id] = true;

        return new TaskHandle(
            fn(): bool => $this->cancelFiber($id),
            fn(): bool => isset($this->fibers[$id]),
        );
    }

    public static function sleep(int $ticks): void
    {
        Fiber::suspend(new Sleep($ticks));
    }

    public function tick(): void
    {
        ++$this->tick;
        $this->runDueTasks();
        $this->startPendingFibers();
        $this->wakeSleepingFibers();
    }

    public function shutdown(): void
    {
        $this->tasks = [];
        $this->pendingFibers = [];
        $this->sleeping = [];
        $this->fibers = [];
        $this->staleTaskEntries = 0;
        $this->taskQueue->clear();
        $this->sleepQueue->clear();
    }

    private function runDueTasks(): void
    {
        while (($due = $this->taskQueue->peekDue()) !== null && $due <= $this->tick) {
            $entry = $this->taskQueue->pop();
            if ($entry === null) {
                return;
            }

            $taskId = $entry['id'];
            $scheduled = $this->tasks[$taskId] ?? null;
            if ($scheduled === null) {
                if ($this->staleTaskEntries > 0) {
                    --$this->staleTaskEntries;
                }
                continue;
            }
            if ($scheduled['due'] !== $entry['due']) {
                continue;
            }

            try {
                ($scheduled['task'])();
            } catch (Throwable $error) {
                unset($this->tasks[$taskId]);
                throw $error;
            }

            if (!isset($this->tasks[$taskId])) {
                continue;
            }
            if ($scheduled['interval'] === null) {
                unset($this->tasks[$taskId]);
                continue;
            }

            $nextDue = $this->tick + $scheduled['interval'];
            $this->tasks[$taskId]['due'] = $nextDue;
            $this->taskQueue->push($nextDue, $taskId);
        }
    }

    private function startPendingFibers(): void
    {
        foreach (array_keys($this->pendingFibers) as $fiberId) {
            unset($this->pendingFibers[$fiberId]);
            $fiber = $this->fibers[$fiberId] ?? null;
            if ($fiber === null) {
                continue;
            }

            try {
                $yielded = $fiber->start();
            } catch (Throwable $error) {
                $this->forgetFiber($fiberId);
                throw $error;
            }
            $this->captureFiberState($fiberId, $yielded);
        }
    }

    private function wakeSleepingFibers(): void
    {
        while (($wake = $this->sleepQueue->peekDue()) !== null && $wake <= $this->tick) {
            $entry = $this->sleepQueue->pop();
            if ($entry === null) {
                return;
            }

            $fiberId = $entry['id'];
            if (($this->sleeping[$fiberId] ?? null) !== $entry['due']) {
                continue;
            }

            unset($this->sleeping[$fiberId]);
            $fiber = $this->fibers[$fiberId] ?? null;
            if ($fiber === null) {
                continue;
            }

            try {
                $yielded = $fiber->resume();
            } catch (Throwable $error) {
                $this->forgetFiber($fiberId);
                throw $error;
            }
            $this->captureFiberState($fiberId, $yielded);
        }
    }

    private function captureFiberState(int $fiberId, mixed $yielded): void
    {
        $fiber = $this->fibers[$fiberId] ?? null;
        if ($fiber === null) {
            return;
        }
        if ($fiber->isTerminated()) {
            $this->forgetFiber($fiberId);

            return;
        }

        if ($yielded instanceof Sleep) {
            $wake = $this->tick + $yielded->ticks;
            $this->sleeping[$fiberId] = $wake;
            $this->sleepQueue->push($wake, $fiberId);

            return;
        }

        $this->forgetFiber($fiberId);
        throw new LogicException(
            'Cobblestone Fiber suspended without Scheduler::sleep()',
        );
    }

    private function scheduledHandle(int $taskId): TaskHandle
    {
        return new TaskHandle(
            fn(): bool => $this->cancelScheduledTask($taskId),
            fn(): bool => isset($this->tasks[$taskId]),
        );
    }

    private function cancelScheduledTask(int $taskId): bool
    {
        if (!isset($this->tasks[$taskId])) {
            return false;
        }

        unset($this->tasks[$taskId]);
        ++$this->staleTaskEntries;
        $this->compactTaskQueueIfNeeded();

        return true;
    }

    private function cancelFiber(int $fiberId): bool
    {
        if (!isset($this->fibers[$fiberId])) {
            return false;
        }

        $this->forgetFiber($fiberId);

        return true;
    }

    private function forgetFiber(int $fiberId): void
    {
        unset(
            $this->fibers[$fiberId],
            $this->pendingFibers[$fiberId],
            $this->sleeping[$fiberId],
        );
    }

    private function compactTaskQueueIfNeeded(): void
    {
        if ($this->tasks === []) {
            $this->taskQueue->clear();
            $this->staleTaskEntries = 0;
            return;
        }

        if ($this->staleTaskEntries < 1_024 || $this->staleTaskEntries < count($this->tasks)) {
            return;
        }

        $this->taskQueue->clear();
        foreach ($this->tasks as $taskId => $scheduled) {
            $this->taskQueue->push($scheduled['due'], $taskId);
        }
        $this->staleTaskEntries = 0;
    }
}
