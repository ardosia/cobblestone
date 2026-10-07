<?php

declare(strict_types=1);

use Cobblestone\Server\Event\ServerStarted;
use Cobblestone\Server\Server;
use Cobblestone\Config\ServerConfig;
use Cobblestone\Server\ServerState;
use Cobblestone\Task\Scheduler;
use Cobblestone\Tests\ServerSmokePlugin;

require_once dirname(__DIR__) . '/bootstrap.php';

function server_fail(string $message): never
{
    fwrite(STDERR, $message . PHP_EOL);
    exit(1);
}

if (!extension_loaded('cobblestone_core_php')) {
    server_fail('cobblestone_core_php extension did not load for server smoke test');
}

$server = Server::create(new ServerConfig('127.0.0.1:0', 4, 'Cobblestone Server Test'));
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

$pluginFile = dirname(__DIR__) . '/fixtures/ServerSmokePlugin.php';
$server->loadPlugin($pluginFile, ServerSmokePlugin::class);

if ((ServerSmokePlugin::$state['enabled'] ?? false) !== true) {
    server_fail('plugin did not enable before server start');
}

$server->start();
if (!$started || $server->state() !== ServerState::Running) {
    server_fail('server did not enter running state through explicit start');
}

if ($server->executeCommand('smoke echo one two') !== 'one two') {
    server_fail('typed command tree did not dispatch plugin command');
}
if ($server->suggestCommand('smoke e') !== ['echo']) {
    server_fail('typed command tree did not suggest plugin subcommand');
}

$server->tick(1);
if ((ServerSmokePlugin::$state['scheduled'] ?? false) !== true) {
    server_fail('scheduled owner-runtime task did not run');
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

$runServer = Server::create(
    new ServerConfig('127.0.0.1:0', 2, 'Cobblestone Run Test'),
);
$runServer->after(
    1,
    static function () use ($runServer): void {
        $runServer->stop('run-smoke');
    },
);
$runTicks = $runServer->run(tickRate: 1_000, nativeEventBudget: 1);
if ($runTicks < 1 || $runServer->state() !== ServerState::Stopped) {
    server_fail('Server::run did not own start/tick/shutdown lifecycle');
}

fwrite(
    STDOUT,
    "cobblestone-server: lifecycle=verified events=verified commands=verified plugin=verified scheduler=verified fiber=owner-resumed shutdown=graceful\n",
);
