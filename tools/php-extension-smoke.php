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

$runtimeId = cobblestone_core_runtime_id();
if (!is_int($runtimeId) || $runtimeId <= 0) {
    fail('runtime identity was not a positive integer');
}

$probe = cobblestone_core_probe_create();
if (!is_int($probe) || $probe <= 0) {
    fail('probe handle was not a positive integer');
}

if (!cobblestone_core_probe_valid($probe)) {
    fail('new probe handle was not live');
}

cobblestone_core_probe_drop($probe);

if (cobblestone_core_probe_valid($probe)) {
    fail('released probe handle remained live');
}

$staleRejected = false;
try {
    cobblestone_core_probe_drop($probe);
} catch (Throwable $error) {
    $staleRejected = str_contains($error->getMessage(), 'invalid or stale');
}

if (!$staleRejected) {
    fail('stale probe handle did not become a PHP exception');
}

printf(
    "cobblestone-core-php: runtime_id=%d stale_handle=rejected\n",
    $runtimeId,
);
