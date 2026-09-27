<?php

declare(strict_types=1);

namespace Cobblestone\Plugin;

use Cobblestone\Kernel\CommandRegistry;
use Cobblestone\Kernel\EventBus;
use Cobblestone\Kernel\Scheduler;

/**
 * Ordinary plugin surface for the C007 single-runtime kernel.
 *
 * It intentionally contains no RakNet, worker, queue, mutex, thread, or runtime identity objects.
 */
final readonly class PluginContext
{
    public function __construct(
        public EventBus $events,
        public CommandRegistry $commands,
        public Scheduler $scheduler,
    ) {
    }
}
