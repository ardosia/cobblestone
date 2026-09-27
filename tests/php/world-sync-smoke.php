<?php

declare(strict_types=1);

require __DIR__ . '/bootstrap.php';

use Cobblestone\Server\Server;
use Cobblestone\Server\ServerState;
use Cobblestone\Session\Event\SessionSpawned;
use Cobblestone\World\BlockPos;
use Cobblestone\World\BlockStateId;
use Cobblestone\World\ChunkPos;

function worldSyncExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

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
if (!is_string($bind) || $bind === '') {
    throw new RuntimeException('failed to resolve loopback UDP address');
}

$server = Server::start($bind, 4, 'Cobblestone World Sync Test');
$spawned = false;
$mutated = false;
$server->events()->listen(
    SessionSpawned::class,
    static function (SessionSpawned $event) use ($server, &$spawned, &$mutated): void {
        $spawned = true;
        worldSyncExpect($event->chunksSent > 0, 'world-sync client spawned without chunks');
        $store = $server->world()->nativeStore();
        worldSyncExpect($store !== null, 'world-sync server did not use native world storage');
        worldSyncExpect(
            $store->chunkPinCount(new ChunkPos(8, 8)) > 0,
            'spawned client view did not pin its streamed center chunk',
        );
        $previous = $server->world()->setBlockStateId(
            new BlockPos(128, 5, 128),
            BlockStateId::fromLegacy(1),
        );
        worldSyncExpect(
            $previous === BlockStateId::fromLegacy(0),
            'world-sync mutation expected air before stone',
        );
        $mutated = true;
    },
);

$root = dirname(__DIR__, 2);
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
];
$descriptors = [
    0 => ['pipe', 'r'],
    1 => ['pipe', 'w'],
    2 => ['pipe', 'w'],
];
$process = proc_open($command, $descriptors, $pipes, $root);
if (!is_resource($process)) {
    $server->requestStop('world-sync-client-start-failed');
    $server->stop();
    throw new RuntimeException('failed to start world-sync loopback client');
}
fclose($pipes[0]);
stream_set_blocking($pipes[1], false);
stream_set_blocking($pipes[2], false);

$stdout = '';
$stderr = '';
$exitCode = null;
$deadline = hrtime(true) + 20_000_000_000;

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
            "world-sync loopback client timed out\nstdout={$stdout}\nstderr={$stderr}",
        );
    }

    $stdout .= stream_get_contents($pipes[1]);
    $stderr .= stream_get_contents($pipes[2]);

    worldSyncExpect($exitCode === 0, "world-sync client failed: {$stderr}");
    worldSyncExpect($spawned, 'world-sync session never reached spawned state');
    worldSyncExpect($mutated, 'world-sync mutation was not applied');
    worldSyncExpect(
        str_contains($stdout, 'world-sync-client: update=verified'),
        "world-sync client did not observe UpdateBlock\nstdout={$stdout}\nstderr={$stderr}",
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
        $server->requestStop('world-sync-smoke');
        $server->stop();
    }
}

$store = $server->world()->nativeStore();
worldSyncExpect($store !== null, 'world-sync native store disappeared during shutdown');
worldSyncExpect(
    $store->chunkPinCount(new ChunkPos(8, 8)) === 0,
    'session shutdown did not release streamed chunk pins',
);

fwrite(STDOUT, "world-sync-smoke: passed\n");
