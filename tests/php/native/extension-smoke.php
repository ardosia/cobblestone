<?php

declare(strict_types=1);

function fail(string $message): never
{
    fwrite(STDERR, $message . PHP_EOL);
    exit(1);
}

if (!extension_loaded('cobblestone_core_php')) {
    fail('cobblestone_core_php extension did not load');
}

$abi = cobblestone_core_abi();
if ($abi !== 2) {
    fail('native ABI version mismatch');
}

$runtimeId = cobblestone_core_runtime_id();
if (!is_int($runtimeId) || $runtimeId <= 0) {
    fail('runtime identity was not a positive integer');
}

if (cobblestone_core_runtime_id() !== $runtimeId) {
    fail('runtime identity was not stable within the PHP runtime');
}

printf(
    "cobblestone-core-php: abi=%d runtime_id=%d identity=stable\n",
    $abi,
    $runtimeId,
);
