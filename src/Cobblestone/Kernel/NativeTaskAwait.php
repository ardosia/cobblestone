<?php

declare(strict_types=1);

namespace Cobblestone\Kernel;

use InvalidArgumentException;

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
