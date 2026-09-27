<?php

declare(strict_types=1);

namespace Cobblestone\Tick;

use InvalidArgumentException;

final readonly class TickLoopConfig
{
    public function __construct(
        public int $tickRate = 20,
        public int $warningThresholdMillis = 2_000,
        public int $warningIntervalMillis = 15_000,
        public int $maxCatchUpTicks = 10,
    ) {
        if ($tickRate <= 0 || $tickRate > 1_000) {
            throw new InvalidArgumentException('tick rate must be between 1 and 1000');
        }
        if ($warningThresholdMillis < 0) {
            throw new InvalidArgumentException('tick warning threshold cannot be negative');
        }
        if ($warningIntervalMillis <= 0) {
            throw new InvalidArgumentException('tick warning interval must be positive');
        }
        if ($maxCatchUpTicks <= 0) {
            throw new InvalidArgumentException('max catch-up ticks must be positive');
        }
    }

    public function periodNanos(): int
    {
        return intdiv(1_000_000_000, $this->tickRate);
    }

    public function warningThresholdNanos(): int
    {
        return $this->warningThresholdMillis * 1_000_000;
    }

    public function warningIntervalNanos(): int
    {
        return $this->warningIntervalMillis * 1_000_000;
    }
}
