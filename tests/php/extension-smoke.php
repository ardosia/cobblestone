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

$panicContained = false;
try {
    cobblestone_core_probe_panic();
} catch (Throwable $error) {
    $panicContained = str_contains($error->getMessage(), 'native panic contained');
}

if (!$panicContained) {
    fail('deliberate native panic did not become a PHP exception');
}

if (cobblestone_core_runtime_id() !== $runtimeId) {
    fail('runtime identity changed after contained panic');
}

$postPanicProbe = cobblestone_core_probe_create();
if (!cobblestone_core_probe_valid($postPanicProbe)) {
    fail('extension state was unusable after contained panic');
}
cobblestone_core_probe_drop($postPanicProbe);

printf(
    "cobblestone-core-php: runtime_id=%d stale_handle=rejected panic=contained\n",
    $runtimeId,
);
