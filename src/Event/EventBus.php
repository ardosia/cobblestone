<?php

declare(strict_types=1);

namespace Cobblestone\Event;

use Closure;
use InvalidArgumentException;

final class EventBus
{
    /** @var array<class-string, array<int, Closure(object): void>> */
    private array $listeners = [];

    /** @var array<class-string, list<Closure(object): void>> */
    private array $resolved = [];

    private int $nextListenerId = 1;

    /** @param class-string $event */
    public function listen(string $event, Closure $listener): Subscription
    {
        if ($event === '') {
            throw new InvalidArgumentException('event class must not be empty');
        }

        $id = $this->nextListenerId++;
        $this->listeners[$event][$id] = $listener;
        $this->resolved = [];

        return new Subscription(
            fn (): bool => $this->remove($event, $id),
        );
    }

    public function dispatch(object $event): void
    {
        foreach ($this->resolvedListeners($event::class) as $listener) {
            $listener($event);
        }
    }

    /** @param class-string $event */
    private function remove(string $event, int $id): bool
    {
        if (!isset($this->listeners[$event][$id])) {
            return false;
        }

        unset($this->listeners[$event][$id]);
        if ($this->listeners[$event] === []) {
            unset($this->listeners[$event]);
        }
        $this->resolved = [];

        return true;
    }

    /**
     * @param class-string $event
     * @return list<Closure(object): void>
     */
    private function resolvedListeners(string $event): array
    {
        if (isset($this->resolved[$event])) {
            return $this->resolved[$event];
        }

        $resolved = [];
        foreach ($this->listeners as $class => $listeners) {
            if ($event !== $class && !is_a($event, $class, true)) {
                continue;
            }

            foreach ($listeners as $listener) {
                $resolved[] = $listener;
            }
        }

        return $this->resolved[$event] = $resolved;
    }
}
