<?php

declare(strict_types=1);

namespace Cobblestone\Kernel;

use Closure;
use InvalidArgumentException;

/**
 * Synchronous owner-runtime event dispatcher.
 *
 * Listeners always run on the owning PHP runtime. Native transport/wire events must be translated
 * into gameplay/kernel events before reaching this surface.
 */
final class EventBus
{
    /** @var array<class-string, list<Closure(object): void>> */
    private array $listeners = [];

    /**
     * @param class-string $eventClass
     * @param Closure(object): void $listener
     */
    public function listen(string $eventClass, Closure $listener): void
    {
        if (!class_exists($eventClass) && !interface_exists($eventClass)) {
            throw new InvalidArgumentException("event class does not exist: {$eventClass}");
        }

        $this->listeners[$eventClass][] = $listener;
    }

    public function dispatch(object $event): void
    {
        foreach ($this->listeners as $eventClass => $listeners) {
            if (!$event instanceof $eventClass) {
                continue;
            }

            foreach ($listeners as $listener) {
                $listener($event);
            }
        }
    }
}
