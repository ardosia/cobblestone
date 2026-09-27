<?php

declare(strict_types=1);

namespace Cobblestone\Tick;

final class SystemClock implements Clock
{
    public function nowNanos(): int
    {
        return hrtime(true);
    }

    public function sleepNanos(int $nanos): void
    {
        if ($nanos <= 0) {
            return;
        }

        $microseconds = intdiv($nanos, 1_000);
        if ($microseconds > 0) {
            usleep($microseconds);
        }
    }
}
