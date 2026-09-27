<?php

declare(strict_types=1);

require __DIR__ . '/bootstrap.php';

use Cobblestone\Tick\Clock;
use Cobblestone\Tick\TickLoop;
use Cobblestone\Tick\TickLoopConfig;
use Psr\Log\AbstractLogger;

$clock = new class implements Clock {
    public int $now = 0;

    public function nowNanos(): int
    {
        return $this->now;
    }

    public function sleepNanos(int $nanos): void
    {
        $this->now += max(0, $nanos);
    }

    public function advance(int $nanos): void
    {
        $this->now += $nanos;
    }
};

$logger = new class extends AbstractLogger {
    /** @var list<array{level: mixed, message: string, context: array<string, mixed>}> */
    public array $records = [];

    public function log($level, string|\Stringable $message, array $context = []): void
    {
        $this->records[] = [
            'level' => $level,
            'message' => (string) $message,
            'context' => $context,
        ];
    }
};

$loop = new TickLoop(
    new TickLoopConfig(
        tickRate: 20,
        warningThresholdMillis: 100,
        warningIntervalMillis: 1_000,
        maxCatchUpTicks: 4,
    ),
    $logger,
    $clock,
);

$executed = 0;
$count = $loop->run(
    static function (int $tick) use (&$executed, $clock): void {
        ++$executed;
        if ($tick === 1) {
            $clock->advance(250_000_000);
        }
    },
    static function () use (&$executed): bool {
        return $executed < 3;
    },
);

if ($count !== 3 || $executed !== 3) {
    throw new RuntimeException('tick loop did not execute the expected number of ticks');
}

$warnings = array_values(array_filter(
    $logger->records,
    static fn (array $record): bool => str_starts_with(
        $record['message'],
        "Can't keep up! Is the server overloaded?",
    ),
));

if ($warnings === []) {
    throw new RuntimeException('tick loop did not report an overload warning');
}
if (($warnings[0]['context']['ticks_behind'] ?? 0) < 1) {
    throw new RuntimeException('tick overload warning did not include ticks-behind context');
}

fwrite(STDOUT, "tick-smoke: passed\n");
