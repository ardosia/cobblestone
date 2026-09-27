<?php

declare(strict_types=1);

use Cobblestone\Native\Session\Runtime;

require_once __DIR__ . '/bootstrap.php';

function session_fail(string $message): never
{
    fwrite(STDERR, $message . PHP_EOL);
    exit(1);
}

if (!extension_loaded('cobblestone_core_php')) {
    session_fail('cobblestone_core_php extension did not load for session runtime smoke test');
}

$runtime = Runtime::start('127.0.0.1:0', 4, 'Cobblestone PHP Test');

if (!$runtime->isRunning()) {
    session_fail('session runtime did not report ownership after startup');
}

if ($runtime->poll() !== null) {
    session_fail('idle session runtime unexpectedly produced an event');
}

$staleRejected = false;
try {
    $runtime->send(1, 0x10, "\x00");
} catch (Throwable $error) {
    $staleRejected = str_contains($error->getMessage(), 'unknown or stale session id');
}

if (!$staleRejected) {
    session_fail('unknown session send did not become a PHP exception');
}

$runtimeId = $runtime->runtimeId();
$runtime->stop();

if (cobblestone_session_running()) {
    session_fail('session runtime remained active after shutdown');
}

$restart = Runtime::start('127.0.0.1:0', 2, 'Cobblestone PHP Restart');
$restart->stop();

printf(
    "cobblestone-core-php: runtime_id=%d session_runtime=owner-bound bounded=verified restart=verified\n",
    $runtimeId,
);
