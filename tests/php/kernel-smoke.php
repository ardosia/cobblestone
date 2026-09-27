<?php

declare(strict_types=1);

use Cobblestone\Kernel\Scheduler;
use Cobblestone\Kernel\ServerKernel;
use Cobblestone\Tests\KernelSmokePlugin;

require_once dirname(__DIR__, 2) . '/src/Cobblestone/Internal/NativeSessionRuntime.php';
require_once dirname(__DIR__, 2) . '/src/Cobblestone/Internal/Protocol84Bootstrap.php';
require_once dirname(__DIR__, 2) . '/src/Cobblestone/Kernel/EventBus.php';
require_once dirname(__DIR__, 2) . '/src/Cobblestone/Kernel/CommandRegistry.php';
require_once dirname(__DIR__, 2) . '/src/Cobblestone/Kernel/Scheduler.php';
require_once dirname(__DIR__, 2) . '/src/Cobblestone/Plugin/Plugin.php';
require_once dirname(__DIR__, 2) . '/src/Cobblestone/Plugin/PluginContext.php';
require_once dirname(__DIR__, 2) . '/src/Cobblestone/Plugin/PluginManager.php';
require_once dirname(__DIR__, 2) . '/src/Cobblestone/Kernel/ServerKernel.php';

function kernel_fail(string $message): never
{
    fwrite(STDERR, $message . PHP_EOL);
    exit(1);
}

if (!extension_loaded('cobblestone_core_php')) {
    kernel_fail('cobblestone_core_php extension did not load for kernel smoke test');
}

$kernel = ServerKernel::start('127.0.0.1:0', 4, 'Cobblestone Kernel Test');

$pluginFile = __DIR__ . '/fixtures/KernelSmokePlugin.php';
$kernel->plugins()->load($pluginFile, KernelSmokePlugin::class);

if ((KernelSmokePlugin::$state['enabled'] ?? false) !== true) {
    kernel_fail('plugin did not enable');
}

if ($kernel->commands()->execute('smoke:echo', ['one', 'two']) !== 'one:two') {
    kernel_fail('command registry did not dispatch plugin command');
}

$kernel->tick(1);
if ((KernelSmokePlugin::$state['scheduled'] ?? false) !== true) {
    kernel_fail('scheduled owner-runtime task did not run');
}

$nativeTask = cobblestone_core_async_submit(21);
$fiberResult = null;

$kernel->scheduler()->spawn(
    static function () use ($nativeTask, &$fiberResult): void {
        $fiberResult = Scheduler::awaitNative($nativeTask);
    },
);

$deadline = hrtime(true) + 5_000_000_000;
while ($fiberResult === null) {
    $kernel->tick(1);
    if (hrtime(true) >= $deadline) {
        kernel_fail('Fiber native completion did not arrive before deadline');
    }
    usleep(1_000);
}

if ($fiberResult !== 42) {
    kernel_fail('Fiber native completion returned the wrong value');
}

$slept = false;
$kernel->scheduler()->spawn(
    static function () use (&$slept): void {
        Scheduler::sleep(2);
        $slept = true;
    },
);

$kernel->tick(1);
if ($slept) {
    kernel_fail('Fiber tick sleep resumed too early');
}

$kernel->tick(1);
if (!$slept) {
    kernel_fail('Fiber tick sleep did not resume on owner runtime');
}

$kernel->stop();

if ((KernelSmokePlugin::$state['stopping'] ?? false) !== true) {
    kernel_fail('server stopping event was not dispatched');
}
if ((KernelSmokePlugin::$state['disabled'] ?? false) !== true) {
    kernel_fail('plugin did not disable');
}
if (cobblestone_session_running()) {
    kernel_fail('native session runtime remained active after kernel stop');
}

printf(
    "cobblestone-kernel: lifecycle=verified events=verified commands=verified plugin=verified scheduler=verified fiber=owner-resumed\n",
);
