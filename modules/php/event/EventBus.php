<?php

declare(strict_types=1);

namespace Cobblestone\Event;

use Closure;
use InvalidArgumentException;

final class EventBus
{
    /** @var array<class-string, list<Closure(object): void>> */
    private array $listeners = [];

    /** @param class-string $event */
    public function listen(string $event, Closure $listener): void
    {
        if ($event === '') {
            throw new InvalidArgumentException('event class must not be empty');
        }
        $this->listeners[$event][] = $listener;
    }

    public function dispatch(object $event): void
    {
        foreach ($this->listeners as $class => $listeners) {
            if (!$event instanceof $class) {
                continue;
            }
            foreach ($listeners as $listener) {
                $listener($event);
            }
        }
    }
}
