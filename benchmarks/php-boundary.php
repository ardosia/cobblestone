<?php

declare(strict_types=1);

function bench_fail(string $message): never
{
    fwrite(STDERR, $message . PHP_EOL);
    exit(1);
}

function print_bench(string $name, int $iterations, int $elapsedNs, string $extra = ''): void
{
    $perOp = $elapsedNs / $iterations;
    printf(
        "php_bench name=%s iterations=%d total_ns=%d ns_per_op=%.2f%s\n",
        $name,
        $iterations,
        $elapsedNs,
        $perOp,
        $extra,
    );
}

if (!extension_loaded('cobblestone_core_php')) {
    bench_fail('cobblestone_core_php extension did not load for benchmark');
}

$pingIterations = 100_000;
for ($i = 0; $i < 1_000; ++$i) {
    cobblestone_core_ping();
}

$started = hrtime(true);
$checksum = 0;
for ($i = 0; $i < $pingIterations; ++$i) {
    $checksum += cobblestone_core_ping();
}
$elapsed = hrtime(true) - $started;
print_bench('ffi_empty', $pingIterations, $elapsed);
if ($checksum !== 0) {
    bench_fail('empty-call benchmark checksum changed');
}

$probe = cobblestone_core_probe_create();
$handleIterations = 100_000;
$started = hrtime(true);
$validCount = 0;
for ($i = 0; $i < $handleIterations; ++$i) {
    if (cobblestone_core_probe_valid($probe)) {
        ++$validCount;
    }
}
$elapsed = hrtime(true) - $started;
print_bench('handle_lookup_php_boundary', $handleIterations, $elapsed);
if ($validCount !== $handleIterations) {
    bench_fail('handle benchmark observed an invalid live probe');
}
cobblestone_core_probe_drop($probe);

$bufferCases = [
    [0, 50_000],
    [64, 20_000],
    [1_024, 5_000],
    [16_384, 500],
    [65_536, 100],
];

foreach ($bufferCases as [$size, $iterations]) {
    $payload = str_repeat('x', $size);
    $started = hrtime(true);

    for ($i = 0; $i < $iterations; ++$i) {
        if (cobblestone_core_buffer_copy_len($payload) !== $size) {
            bench_fail('native buffer benchmark returned an incorrect length');
        }
    }

    $elapsed = hrtime(true) - $started;
    $explicitBytes = $size * $iterations;
    print_bench(
        'buffer_copy_php_boundary',
        $iterations,
        $elapsed,
        sprintf(' size=%d explicit_native_bytes=%d', $size, $explicitBytes),
    );
}

$batchSize = 32;
$batches = 100;
$submitted = 0;
$submissionNs = 0;
$workerChecksum = 0;

for ($batch = 0; $batch < $batches; ++$batch) {
    $tasks = [];
    $started = hrtime(true);
    for ($i = 0; $i < $batchSize; ++$i) {
        $tasks[] = cobblestone_core_async_submit($i);
    }
    $submissionNs += hrtime(true) - $started;
    $submitted += $batchSize;

    foreach ($tasks as $task) {
        while (!cobblestone_core_async_ready($task)) {
            // Deliberately nonblocking: the owning PHP thread stays in control.
        }
        $workerChecksum += cobblestone_core_async_take($task);
    }
}

print_bench('worker_submit_php_boundary', $submitted, $submissionNs);
if ($workerChecksum <= 0) {
    bench_fail('worker submission benchmark checksum was invalid');
}

$fiberIterations = 500;
$fiber = new Fiber(
    function () use ($fiberIterations): int {
        $sum = 0;
        for ($i = 0; $i < $fiberIterations; ++$i) {
            $task = cobblestone_core_async_submit($i);
            $result = Fiber::suspend($task);
            if (!is_int($result) || $result !== $i * 2) {
                bench_fail('Fiber benchmark received an incorrect native completion');
            }
            $sum += $result;
        }
        return $sum;
    },
);

$started = hrtime(true);
$task = $fiber->start();
$deadline = hrtime(true) + 10_000_000_000;

while (!$fiber->isTerminated()) {
    if (!is_int($task)) {
        bench_fail('Fiber benchmark suspended without a task identity');
    }

    while (!cobblestone_core_async_ready($task)) {
        if (hrtime(true) >= $deadline) {
            bench_fail('Fiber benchmark native completion deadline expired');
        }
    }

    $result = cobblestone_core_async_take($task);
    $task = $fiber->resume($result);
}

$elapsed = hrtime(true) - $started;
print_bench('fiber_completion_wake', $fiberIterations, $elapsed);
if ($fiber->getReturn() <= 0) {
    bench_fail('Fiber benchmark checksum was invalid');
}
