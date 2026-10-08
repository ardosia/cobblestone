<?php

declare(strict_types=1);

namespace Cobblestone\Server\Internal\Timing;

interface Clock
{
    public function nowNanos(): int;

    public function sleepNanos(int $nanos): void;
}
