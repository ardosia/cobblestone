<?php

declare(strict_types=1);

function fiber_fail(string $message): never
{
    fwrite(STDERR, $message . PHP_EOL);
    exit(1);
}

if (!extension_loaded('cobblestone_core_php')) {
    fiber_fail('cobblestone_core_php extension did not load for Fiber smoke test');
}

$runtimeId = cobblestone_core_runtime_id();
$task = cobblestone_core_async_submit(21);

if (!is_int($task) || $task <= 0) {
    fiber_fail('native async task identity was not a positive integer');
}

$fiber = new Fiber(
    function () use ($runtimeId, $task): int {
        if (cobblestone_core_runtime_id() !== $runtimeId) {
            fiber_fail('runtime identity changed inside Fiber');
        }

        $value = Fiber::suspend($task);
        if (!is_int($value)) {
            fiber_fail('Fiber resumed with a non-integer completion');
        }

        return $value;
    },
);

$waitingFor = $fiber->start();

if ($waitingFor !== $task || !$fiber->isSuspended()) {
    fiber_fail('Fiber did not suspend on the submitted native task');
}

$deadline = hrtime(true) + 5_000_000_000;

while (!$fiber->isTerminated()) {
    if (cobblestone_core_async_ready($waitingFor)) {
        $value = cobblestone_core_async_take($waitingFor);
        $fiber->resume($value);
        break;
    }

    if (hrtime(true) >= $deadline) {
        fiber_fail('native completion did not arrive before the diagnostic deadline');
    }

    usleep(1_000);
}

if (!$fiber->isTerminated()) {
    fiber_fail('Fiber did not terminate after owner-runtime resume');
}

if ($fiber->getReturn() !== 42) {
    fiber_fail('Fiber observed an incorrect native completion value');
}

printf(
    "cobblestone-core-php: runtime_id=%d task=%d fiber=owner-resumed result=%d\n",
    $runtimeId,
    $task,
    $fiber->getReturn(),
);
