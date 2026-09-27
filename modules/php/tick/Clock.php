<?php

declare(strict_types=1);

namespace Cobblestone\Tick;

interface Clock
{
    public function nowNanos(): int;

    public function sleepNanos(int $nanos): void;
}
