<?php

declare(strict_types=1);

require dirname(__DIR__) . '/bootstrap.php';

use Cobblestone\Task\Scheduler;
use Cobblestone\Task\TaskHandle;

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
schedulerExpect($cancelled->cancel(), 'scheduled task cancellation failed');
schedulerExpect(!$cancelled->active(), 'cancelled task still reported active');

$repeatRuns = 0;
$repeat = null;
$repeat = $scheduler->repeat(
    2,
    static function () use (&$repeatRuns, &$repeat): void {
        ++$repeatRuns;
        if ($repeatRuns === 2) {
            if (!$repeat instanceof TaskHandle) {
                throw new RuntimeException('repeating task handle was not initialized');
            }
            $repeat->cancel();
        }
    },
);

$started = false;
$slept = false;
$fiber = $scheduler->spawn(
    static function () use (&$started, &$slept): void {
        $started = true;
        Scheduler::sleep(3);
        $slept = true;
    },
);

schedulerExpect(!$started, 'spawned Fiber started inline instead of at scheduler boundary');
schedulerExpect($fiber->active(), 'queued Fiber did not report active');

$scheduler->tick();
schedulerExpect($order === [1, 2], 'equal-due tasks did not retain stable task-id order');
schedulerExpect($started, 'spawned Fiber did not start on the next scheduler tick');
schedulerExpect(!$slept, 'sleeping Fiber resumed too early');
schedulerExpect($repeatRuns === 0, 'repeating task ran too early');

$scheduler->tick();
schedulerExpect($repeatRuns === 1, 'repeating task missed first due tick');
schedulerExpect(!$slept, 'sleeping Fiber resumed before wake tick');

$scheduler->tick();
schedulerExpect(!$slept, 'sleeping Fiber resumed one tick early');

$scheduler->tick();
schedulerExpect($slept, 'sleeping Fiber did not resume on wake tick');
schedulerExpect(!$fiber->active(), 'terminated Fiber still reported active');
schedulerExpect($repeatRuns === 2, 'repeating task missed second due tick');
schedulerExpect($unexpected === 0, 'dormant or cancelled work executed unexpectedly');

$scheduler->tick();
schedulerExpect($repeatRuns === 2, 'cancelled repeating task ran again');

$failed = null;
$failed = $scheduler->repeat(
    1,
    static function (): void {
        throw new RuntimeException('expected scheduler failure');
    },
);
try {
    $scheduler->tick();
    throw new RuntimeException('failing repeating task did not throw');
} catch (RuntimeException $error) {
    schedulerExpect($error->getMessage() === 'expected scheduler failure', 'unexpected scheduler failure');
}
schedulerExpect(!$failed->active(), 'failed repeating task remained active');

$scheduler->shutdown();

fwrite(STDOUT, "scheduler-smoke: passed\n");
