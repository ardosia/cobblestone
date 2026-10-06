<?php

declare(strict_types=1);

use Cobblestone\Application\ApplicationConfig;
use Cobblestone\Config\ServerConfig;
use Cobblestone\Config\StorageConfig;

require dirname(__DIR__, 3) . '/vendor/autoload.php';

function configExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

$root = DIRECTORY_SEPARATOR . 'tmp' . DIRECTORY_SEPARATOR . 'cobblestone';
$defaults = ApplicationConfig::fromEnvironment(
    $root,
    ['COBBLESTONE_WORLD_SEED' => '42'],
);
configExpect($defaults->server->bind === '0.0.0.0:19132', 'default bind mismatch');
configExpect($defaults->server->maxConnections === 20, 'default max connections mismatch');
configExpect($defaults->server->name === 'Cobblestone', 'default server name mismatch');
configExpect($defaults->server->initialChunkRadius === 3, 'default view distance mismatch');
configExpect($defaults->server->tickRate === 20, 'default tick rate mismatch');
configExpect(
    $defaults->world->directory === $root . DIRECTORY_SEPARATOR . 'worlds' . DIRECTORY_SEPARATOR . 'world',
    'default world directory mismatch',
);
configExpect($defaults->world->name === 'Cobblestone', 'default world name mismatch');
configExpect($defaults->world->seed === 42, 'explicit numeric seed mismatch');
configExpect($defaults->storage->saveWorkers === 2, 'default save workers mismatch');
configExpect($defaults->storage->loadWorkers === 2, 'default load workers mismatch');
configExpect(
    $defaults->storage->compactionMinDeadBytes === StorageConfig::DEFAULT_COMPACTION_MIN_DEAD_BYTES,
    'default compaction byte threshold mismatch',
);
configExpect(
    $defaults->storage->compactionMinDeadPercent === StorageConfig::DEFAULT_COMPACTION_MIN_DEAD_PERCENT,
    'default compaction percent threshold mismatch',
);
configExpect($defaults->logLevel === 'INFO', 'default log level mismatch');

$overrides = ApplicationConfig::fromEnvironment(
    $root,
    [
        'COBBLESTONE_BIND' => '127.0.0.1:20000',
        'COBBLESTONE_SERVER_NAME' => 'Test Server',
        'COBBLESTONE_VIEW_DISTANCE' => '2',
        'COBBLESTONE_MAX_CONNECTIONS' => '64',
        'COBBLESTONE_TICK_RATE' => '40',
        'COBBLESTONE_WORLD_DIR' => '/srv/cobblestone/world',
        'COBBLESTONE_WORLD_NAME' => 'Target World',
        'COBBLESTONE_WORLD_SEED' => '-1385905961',
        'COBBLESTONE_SAVE_WORKERS' => '4',
        'COBBLESTONE_LOAD_WORKERS' => '5',
        'COBBLESTONE_COMPACTION_MIN_DEAD_BYTES' => '123456',
        'COBBLESTONE_COMPACTION_MIN_DEAD_PERCENT' => '75',
        'COBBLESTONE_LOG_LEVEL' => 'DEBUG',
    ],
);
configExpect($overrides->server->bind === '127.0.0.1:20000', 'bind override mismatch');
configExpect($overrides->server->maxConnections === 64, 'max connections override mismatch');
configExpect($overrides->server->name === 'Test Server', 'server name override mismatch');
configExpect($overrides->server->initialChunkRadius === 2, 'view distance override mismatch');
configExpect($overrides->server->tickRate === 40, 'tick rate override mismatch');
configExpect($overrides->world->directory === '/srv/cobblestone/world', 'world directory override mismatch');
configExpect($overrides->world->name === 'Target World', 'world name override mismatch');
configExpect($overrides->world->seed === -1_385_905_961, 'world seed override mismatch');
configExpect($overrides->storage->saveWorkers === 4, 'save workers override mismatch');
configExpect($overrides->storage->loadWorkers === 5, 'load workers override mismatch');
configExpect($overrides->storage->compactionMinDeadBytes === 123_456, 'compaction bytes override mismatch');
configExpect($overrides->storage->compactionMinDeadPercent === 75, 'compaction percent override mismatch');
configExpect($overrides->logLevel === 'DEBUG', 'log level override mismatch');

foreach ([
    ['COBBLESTONE_MAX_CONNECTIONS', '0'],
    ['COBBLESTONE_MAX_CONNECTIONS', 'nope'],
    ['COBBLESTONE_VIEW_DISTANCE', '0'],
    ['COBBLESTONE_VIEW_DISTANCE', '4'],
    ['COBBLESTONE_TICK_RATE', '0'],
    ['COBBLESTONE_TICK_RATE', '1001'],
    ['COBBLESTONE_SAVE_WORKERS', '0'],
    ['COBBLESTONE_SAVE_WORKERS', '33'],
    ['COBBLESTONE_LOAD_WORKERS', '0'],
    ['COBBLESTONE_LOAD_WORKERS', '33'],
    ['COBBLESTONE_COMPACTION_MIN_DEAD_BYTES', '-1'],
    ['COBBLESTONE_COMPACTION_MIN_DEAD_BYTES', 'wat'],
    ['COBBLESTONE_COMPACTION_MIN_DEAD_PERCENT', '101'],
] as [$key, $value]) {
    try {
        ApplicationConfig::fromEnvironment(
            $root,
            ['COBBLESTONE_WORLD_SEED' => '42', $key => $value],
        );
        throw new RuntimeException("invalid {$key}={$value} was accepted");
    } catch (InvalidArgumentException) {
    }
}

try {
    new ServerConfig(tickRate: 0);
    throw new RuntimeException('invalid direct ServerConfig tick rate was accepted');
} catch (InvalidArgumentException) {
}

try {
    new StorageConfig(saveWorkers: 33);
    throw new RuntimeException('invalid direct StorageConfig worker count was accepted');
} catch (InvalidArgumentException) {
}

fwrite(STDOUT, "config-smoke: passed\n");
