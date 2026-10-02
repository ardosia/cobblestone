<?php

declare(strict_types=1);

use Cobblestone\Server\Event\ServerStarted;
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

$server = Server::create('127.0.0.1:0', 4, 'Cobblestone Server Test');
if ($server->state() !== ServerState::Created) {
    server_fail('server did not remain created before lifecycle start');
}

$started = false;
$server->on(
    ServerStarted::class,
    static function () use (&$started): void {
        $started = true;
    },
);
if ($started) {
    server_fail('server started event fired during construction');
}

$pluginFile = __DIR__ . '/fixtures/ServerSmokePlugin.php';
$server->loadPlugin($pluginFile, ServerSmokePlugin::class);

if ((ServerSmokePlugin::$state['enabled'] ?? false) !== true) {
    server_fail('plugin did not enable before server start');
}

$server->start();
if (!$started || $server->state() !== ServerState::Running) {
    server_fail('server did not enter running state through explicit start');
}

if ($server->executeCommand('smoke:echo', ['one', 'two']) !== 'one:two') {
    server_fail('command registry did not dispatch plugin command');
}

$server->tick(1);
if ((ServerSmokePlugin::$state['scheduled'] ?? false) !== true) {
    server_fail('scheduled owner-runtime task did not run');
}

$nativeTask = cobblestone_core_async_submit(21);
$fiberResult = null;

$server->task(
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
$server->task(
    static function () use (&$slept): void {
        Scheduler::sleep(2);
        $slept = true;
    },
);

$server->tick(1);
if ($slept) {
    server_fail('Fiber tick sleep resumed during its start tick');
}

$server->tick(1);
if ($slept) {
    server_fail('Fiber tick sleep resumed one tick early');
}

$server->tick(1);
if (!$slept) {
    server_fail('Fiber tick sleep did not resume on owner runtime');
}

$server->stop('smoke-test');
if (!$server->isStopRequested()) {
    server_fail('shutdown request was not observable');
}
$server->shutdown();

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
