<?php

declare(strict_types=1);

namespace Cobblestone\Task;

use InvalidArgumentException;

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
