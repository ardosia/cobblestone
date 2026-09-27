<?php

declare(strict_types=1);

require __DIR__ . '/bootstrap.php';

use Cobblestone\Task\Scheduler;

function schedulerExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

$scheduler = new Scheduler();
$unexpected = 0;

for ($i = 0; $i < 10_000; ++$i) {
    $scheduler->schedule(
        100_000 + $i,
        static function () use (&$unexpected): void {
            ++$unexpected;
        },
    );
}

$order = [];
$scheduler->schedule(1, static function () use (&$order): void {
    $order[] = 1;
});
$scheduler->schedule(1, static function () use (&$order): void {
    $order[] = 2;
});

$cancelled = $scheduler->schedule(1, static function () use (&$unexpected): void {
    ++$unexpected;
});
schedulerExpect($scheduler->cancel($cancelled), 'scheduled task cancellation failed');

$repeatRuns = 0;
$repeatId = 0;
$repeatId = $scheduler->repeat(
    2,
    static function () use ($scheduler, &$repeatRuns, &$repeatId): void {
        ++$repeatRuns;
        if ($repeatRuns === 2) {
            $scheduler->cancel($repeatId);
        }
    },
);

$slept = false;
$scheduler->spawn(
    static function () use (&$slept): void {
        Scheduler::sleep(3);
        $slept = true;
    },
);

$scheduler->tick();
schedulerExpect($order === [1, 2], 'equal-due tasks did not retain stable task-id order');
schedulerExpect(!$slept, 'sleeping Fiber resumed too early');
schedulerExpect($repeatRuns === 0, 'repeating task ran too early');

$scheduler->tick();
schedulerExpect($repeatRuns === 1, 'repeating task missed first due tick');
schedulerExpect(!$slept, 'sleeping Fiber resumed before wake tick');

$scheduler->tick();
schedulerExpect($slept, 'sleeping Fiber did not resume on wake tick');

$scheduler->tick();
schedulerExpect($repeatRuns === 2, 'repeating task missed second due tick');
schedulerExpect($unexpected === 0, 'dormant or cancelled work executed unexpectedly');

$scheduler->tick();
schedulerExpect($repeatRuns === 2, 'cancelled repeating task ran again');

$scheduler->shutdown();

fwrite(STDOUT, "scheduler-smoke: passed\n");
