<?php

declare(strict_types=1);

require __DIR__ . '/bootstrap.php';

use Cobblestone\Command\CommandParseException;
use Cobblestone\Command\CommandRegistry;
use function Cobblestone\Command\literal;
use Cobblestone\Event\EventBus;
use Cobblestone\Log\LoggerFactory;
use Cobblestone\Plugin\PluginManager;
use Cobblestone\Task\Scheduler;
use Cobblestone\Tests\FailingSmokePlugin;

function pluginExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

$events = new EventBus();
$commands = new CommandRegistry();
$scheduler = new Scheduler();
$plugins = new PluginManager(
    $events,
    $commands,
    $scheduler,
    LoggerFactory::console('EMERGENCY'),
);

$subscriptionRuns = 0;
$subscription = $events->listen(
    RuntimeException::class,
    static function () use (&$subscriptionRuns): void {
        ++$subscriptionRuns;
    },
);
$events->dispatch(new RuntimeException('first'));
pluginExpect($subscriptionRuns === 1, 'event subscription did not run');
pluginExpect($subscription->cancel(), 'event subscription cancellation failed');
$events->dispatch(new RuntimeException('second'));
pluginExpect($subscriptionRuns === 1, 'cancelled event subscription ran again');

$binding = $commands->register(
    literal('smoke')->then(
        literal('binding')->executes(static fn (): string => 'ok'),
    ),
);
pluginExpect($commands->execute('smoke binding') === 'ok', 'command binding did not execute');
pluginExpect($binding->cancel(), 'command binding cancellation failed');
try {
    $commands->execute('smoke binding');
    throw new RuntimeException('cancelled command binding remained registered');
} catch (CommandParseException) {
}

try {
    $plugins->load(__DIR__ . '/fixtures/FailingSmokePlugin.php', FailingSmokePlugin::class);
    throw new RuntimeException('failing plugin unexpectedly enabled');
} catch (RuntimeException $error) {
    pluginExpect($error->getMessage() === 'expected plugin enable failure', 'wrong plugin enable failure');
}

try {
    $commands->execute('smoke rollback');
    throw new RuntimeException('failed plugin left its command registered');
} catch (CommandParseException) {
}

$scheduler->tick();
pluginExpect((FailingSmokePlugin::$state['task'] ?? false) !== true, 'failed plugin left its task scheduled');

$plugins->shutdown();
$scheduler->shutdown();

fwrite(STDOUT, "plugin-smoke: passed\n");
