<?php

declare(strict_types=1);

require dirname(__DIR__, 2) . '/vendor/autoload.php';

use Cobblestone\Task\Scheduler;

function schedulerBench(string $name, int $live, int $ticks, Closure $setup): void
{
    $scheduler = new Scheduler();
    $setup($scheduler, $live);

    for ($i = 0; $i < 100; ++$i) {
        $scheduler->tick();
    }

    $started = hrtime(true);
    for ($i = 0; $i < $ticks; ++$i) {
        $scheduler->tick();
    }
    $elapsed = hrtime(true) - $started;

    printf(
        "php_bench name=%s live=%d ticks=%d total_ns=%d ns_per_tick=%.2f\n",
        $name,
        $live,
        $ticks,
        $elapsed,
        $elapsed / $ticks,
    );

    $scheduler->shutdown();
}

$farTask = static function (): void {
};

foreach ([0, 100, 1_000, 10_000] as $live) {
    schedulerBench(
        'scheduler_dormant_tasks',
        $live,
        10_000,
        static function (Scheduler $scheduler, int $count) use ($farTask): void {
            for ($i = 0; $i < $count; ++$i) {
                $scheduler->schedule(1_000_000 + $i, $farTask);
            }
        },
    );
}

$sleeper = static function (): void {
    Scheduler::sleep(1_000_000);
};

foreach ([0, 100, 1_000, 5_000] as $live) {
    schedulerBench(
        'scheduler_sleeping_fibers',
        $live,
        10_000,
        static function (Scheduler $scheduler, int $count) use ($sleeper): void {
            for ($i = 0; $i < $count; ++$i) {
                $scheduler->spawn($sleeper);
            }
        },
    );
}

$scheduler = new Scheduler();
$due = 10_000;
$ran = 0;
$dueTask = static function () use (&$ran): void {
    ++$ran;
};
for ($i = 0; $i < $due; ++$i) {
    $scheduler->schedule(1, $dueTask);
}

$started = hrtime(true);
$scheduler->tick();
$elapsed = hrtime(true) - $started;
if ($ran !== $due) {
    throw new RuntimeException('scheduler due-task benchmark missed ready work');
}

printf(
    "php_bench name=scheduler_due_tasks due=%d total_ns=%d ns_per_due=%.2f\n",
    $due,
    $elapsed,
    $elapsed / $due,
);
