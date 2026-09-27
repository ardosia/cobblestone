<?php

declare(strict_types=1);

use Cobblestone\Server\Server;
use Cobblestone\Server\ServerState;
use Cobblestone\Task\Scheduler;
use Cobblestone\Tests\ServerSmokePlugin;

require_once __DIR__ . '/bootstrap.php';

function server_fail(string $message): never
{
    fwrite(STDERR, $message . PHP_EOL);
    exit(1);
}

if (!extension_loaded('cobblestone_core_php')) {
    server_fail('cobblestone_core_php extension did not load for server smoke test');
}

$server = Server::start('127.0.0.1:0', 4, 'Cobblestone Server Test');

$pluginFile = __DIR__ . '/fixtures/ServerSmokePlugin.php';
$server->plugins()->load($pluginFile, ServerSmokePlugin::class);

if ((ServerSmokePlugin::$state['enabled'] ?? false) !== true) {
    server_fail('plugin did not enable');
}

if ($server->commands()->execute('smoke:echo', ['one', 'two']) !== 'one:two') {
    server_fail('command registry did not dispatch plugin command');
}

$server->tick(1);
if ((ServerSmokePlugin::$state['scheduled'] ?? false) !== true) {
    server_fail('scheduled owner-runtime task did not run');
}

$nativeTask = cobblestone_core_async_submit(21);
$fiberResult = null;

$server->scheduler()->spawn(
    static function () use ($nativeTask, &$fiberResult): void {
        $fiberResult = Scheduler::awaitNative($nativeTask);
    },
);

$deadline = hrtime(true) + 5_000_000_000;
while ($fiberResult === null) {
    $server->tick(1);
    if (hrtime(true) >= $deadline) {
        server_fail('Fiber native completion did not arrive before deadline');
    }
    usleep(1_000);
}

if ($fiberResult !== 42) {
    server_fail('Fiber native completion returned the wrong value');
}

$slept = false;
$server->scheduler()->spawn(
    static function () use (&$slept): void {
        Scheduler::sleep(2);
        $slept = true;
    },
);

$server->tick(1);
if ($slept) {
    server_fail('Fiber tick sleep resumed too early');
}

$server->tick(1);
if (!$slept) {
    server_fail('Fiber tick sleep did not resume on owner runtime');
}

$server->requestStop('smoke-test');
if (!$server->isStopRequested()) {
    server_fail('shutdown request was not observable');
}
$server->stop();

if ($server->state() !== ServerState::Stopped) {
    server_fail('server did not reach stopped state');
}
if ((ServerSmokePlugin::$state['stopping'] ?? false) !== true) {
    server_fail('server stopping event was not dispatched');
}
if ((ServerSmokePlugin::$state['disabled'] ?? false) !== true) {
    server_fail('plugin did not disable');
}
if (cobblestone_session_running()) {
    server_fail('native session runtime remained active after server stop');
}

fwrite(
    STDOUT,
    "cobblestone-server: lifecycle=verified events=verified commands=verified plugin=verified scheduler=verified fiber=owner-resumed shutdown=graceful\n",
);
