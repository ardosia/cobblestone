<?php

declare(strict_types=1);

namespace Cobblestone\Plugin;

use Cobblestone\Command\CommandRegistry;
use Cobblestone\Event\EventBus;
use Cobblestone\Task\Scheduler;

final readonly class PluginContext
{
    public function __construct(
        public EventBus $events,
        public CommandRegistry $commands,
        public Scheduler $scheduler,
    ) {
    }
}
