<?php

declare(strict_types=1);

namespace Cobblestone\Task\Internal;

use InvalidArgumentException;

/** @internal */
final readonly class NativeAwait
{
    public function __construct(public int $taskId)
    {
        if ($taskId <= 0) {
            throw new InvalidArgumentException('native task id must be positive');
        }
    }
}
