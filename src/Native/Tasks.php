<?php

declare(strict_types=1);

namespace Cobblestone\Native;

/** @internal Native async-task completion boundary used by the owner-runtime scheduler. */
final class Tasks
{
    public static function ready(int $taskId): bool
    {
        return cobblestone_core_async_ready($taskId);
    }

    public static function take(int $taskId): int
    {
        return cobblestone_core_async_take($taskId);
    }
}
