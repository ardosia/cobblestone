<?php

declare(strict_types=1);

require dirname(__DIR__) . '/bootstrap.php';

$wideInitial = in_array('--wide-initial', $argv, true);

use Cobblestone\Config\StorageConfig;
use Cobblestone\Server\Server;
use Cobblestone\Config\ServerConfig;
use Cobblestone\Server\ServerState;
use Cobblestone\Server\WorldFactory;
use Cobblestone\Session\Event\SessionSpawned;
use Cobblestone\World\Generator\GeneratorType;

function persistentInfiniteJoinExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

function persistentInfiniteJoinRemoveTree(string $root): void
{
    if (!is_dir($root)) {
        return;
    }

    $iterator = new RecursiveIteratorIterator(
        new RecursiveDirectoryIterator($root, FilesystemIterator::SKIP_DOTS),
        RecursiveIteratorIterator::CHILD_FIRST,
    );
    foreach ($iterator as $entry) {
        $entry->isDir() ? rmdir($entry->getPathname()) : unlink($entry->getPathname());
    }
    rmdir($root);
}

$storageRoot = sys_get_temp_dir()
    . '/cobblestone-persistent-infinite-join-'
    . getmypid()
    . '-'
    . bin2hex(random_bytes(4));

$probe = stream_socket_server(
    'udp://127.0.0.1:0',
    $errorCode,
    $errorMessage,
    STREAM_SERVER_BIND,
);
if ($probe === false) {
    throw new RuntimeException("failed to allocate loopback UDP port: {$errorCode} {$errorMessage}");
}
$bind = stream_socket_get_name($probe, false);
fclose($probe);
persistentInfiniteJoinExpect(is_string($bind) && $bind !== '', 'failed to resolve loopback UDP address');

$world = WorldFactory::persistentInfinite(
    $storageRoot,
    'Persistent Infinite Join',
    -1_385_905_961,
    storage: new StorageConfig(saveWorkers: 2, loadWorkers: 2),
);
persistentInfiniteJoinExpect(
    $world->generatorType() === GeneratorType::Infinite,
    'persistent Infinite join did not create an Infinite world',
);

$server = Server::create(
    new ServerConfig(
        bind: $bind,
        maxConnections: 4,
        name: 'Cobblestone Persistent Infinite Join Test',
        initialChunkRadius: $wideInitial ? 3 : 2,
    ),
    world: $world,
);

$spawned = false;
$spawnedChunks = 0;
$server->on(
    SessionSpawned::class,
    static function (SessionSpawned $event) use (&$spawned, &$spawnedChunks): void {
        $spawned = true;
        $spawnedChunks = $event->chunksSent;
    },
);

$server->start();

$root = dirname(__DIR__, 3);
$command = [
    'cargo',
    'run',
    '--quiet',
    '-p',
    'cobblestone-client-bootstrap',
    '--bin',
    'world-sync-client',
    '--',
    $bind,
    '--spawn-only',
    '--expect-spawn=4,63,4',
];
if ($wideInitial) {
    $command[] = '--initial-radius=3';
}
$descriptors = [
    0 => ['pipe', 'r'],
    1 => ['pipe', 'w'],
    2 => ['pipe', 'w'],
];
$process = proc_open($command, $descriptors, $pipes, $root);
if (!is_resource($process)) {
    $server->stop('persistent-infinite-join-client-start-failed');
    $server->shutdown();
    $world->nativeStore()?->destroy();
    persistentInfiniteJoinRemoveTree($storageRoot);
    throw new RuntimeException('failed to start persistent Infinite loopback client');
}
fclose($pipes[0]);
stream_set_blocking($pipes[1], false);
stream_set_blocking($pipes[2], false);

$stdout = '';
$stderr = '';
$exitCode = null;
$deadline = hrtime(true) + 30_000_000_000;

try {
    while (hrtime(true) < $deadline) {
        $server->tick(256);
        $stdout .= stream_get_contents($pipes[1]);
        $stderr .= stream_get_contents($pipes[2]);

        $status = proc_get_status($process);
        if (!$status['running']) {
            $exitCode = $status['exitcode'];
            break;
        }
        usleep(1_000);
    }

    if ($exitCode === null) {
        proc_terminate($process);
        throw new RuntimeException(
            "persistent Infinite join client timed out\nstdout={$stdout}\nstderr={$stderr}",
        );
    }

    $stdout .= stream_get_contents($pipes[1]);
    $stderr .= stream_get_contents($pipes[2]);

    persistentInfiniteJoinExpect(
        $exitCode === 0,
        "persistent Infinite join client failed\nstdout={$stdout}\nstderr={$stderr}",
    );
    persistentInfiniteJoinExpect($spawned, 'persistent Infinite session never reached spawned state');
    persistentInfiniteJoinExpect($spawnedChunks === ($wideInitial ? 49 : 25), 'persistent Infinite join sent wrong initial chunk count');
    persistentInfiniteJoinExpect(
        str_contains($stdout, 'world-sync-client: spawn-position=verified x=4 y=63 z=4'),
        "persistent Infinite client observed wrong StartGame spawn\nstdout={$stdout}\nstderr={$stderr}",
    );
    if ($wideInitial) {
        persistentInfiniteJoinExpect(
            str_contains($stdout, 'world-sync-client: initial-radius=verified radius=3 chunks=49'),
            "persistent Infinite client missed radius-3 chunk coverage\nstdout={$stdout}\nstderr={$stderr}",
        );
    }
    persistentInfiniteJoinExpect(
        str_contains($stdout, 'world-sync-client: spawn=verified'),
        "persistent Infinite client never observed PLAY_STATUS spawned\nstdout={$stdout}\nstderr={$stderr}",
    );
} finally {
    foreach ([1, 2] as $pipe) {
        if (isset($pipes[$pipe]) && is_resource($pipes[$pipe])) {
            fclose($pipes[$pipe]);
        }
    }
    if (is_resource($process)) {
        proc_close($process);
    }
    if ($server->state() === ServerState::Running) {
        $server->stop('persistent-infinite-join-smoke');
        $server->shutdown();
    }
    $world->nativeStore()?->destroy();
    persistentInfiniteJoinRemoveTree($storageRoot);
}

fwrite(STDOUT, "persistent-infinite-join-smoke: passed\n");
