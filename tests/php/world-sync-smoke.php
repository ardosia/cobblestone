<?php

declare(strict_types=1);

require __DIR__ . '/bootstrap.php';

$transitionOnly = in_array('--transition-only', $argv, true);
$radiusCycle = in_array('--radius-cycle', $argv, true);

use Cobblestone\Native\Session\Packet;
use Cobblestone\Server\Server;
use Cobblestone\Server\ServerState;
use Cobblestone\Session\Event\SessionSpawned;
use Cobblestone\World\BlockPos;
use Cobblestone\World\BlockStateId;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\NativeWorldStore;

function worldSyncExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

function worldSyncPinCountOrZero(NativeWorldStore $store, ChunkPos $position): int
{
    try {
        return $store->chunkPinCount($position);
    } catch (Throwable) {
        return 0;
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

$movementHandled = false;
$server = null;
$server = Server::start(
    $bind,
    4,
    'Cobblestone World Sync Test',
    static function (Packet $packet) use (&$movementHandled, &$server): void {
        if ($packet->packetId !== 0x10) {
            return;
        }

        worldSyncExpect($server instanceof Server, 'world-sync server was unavailable to gameplay handler');
        $store = $server->world()->nativeStore();
        worldSyncExpect($store !== null, 'world-sync gameplay handler lost native store');
        worldSyncExpect(
            $store->chunkPinCount(new ChunkPos(8, 8)) > 0,
            'movement preparation released the old streamed center',
        );
        worldSyncExpect(
            $store->chunkPinCount(new ChunkPos(11, 8)) > 0,
            'movement preparation did not pin an entering chunk',
        );
        $movementHandled = true;
    },
    null,
    null,
    $radiusCycle ? 3 : 2,
);
$spawned = false;
$mutated = false;
$server->events()->listen(
    SessionSpawned::class,
    static function (SessionSpawned $event) use ($server, $transitionOnly, $radiusCycle, &$spawned, &$mutated): void {
        $spawned = true;
        worldSyncExpect($event->chunksSent > 0, 'world-sync client spawned without chunks');
        $store = $server->world()->nativeStore();
        worldSyncExpect($store !== null, 'world-sync server did not use native world storage');
        worldSyncExpect(
            $store->chunkPinCount(new ChunkPos(8, 8)) > 0,
            'spawned client view did not pin its streamed center chunk',
        );
        if ($transitionOnly || $radiusCycle) {
            return;
        }

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
    $radiusCycle ? '--radius-cycle' : ($transitionOnly ? '--transition-only' : '--move-after-update'),
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
$transitionCommitted = false;
$radiusCycleCommitted = false;

try {
    while (hrtime(true) < $deadline) {
        $server->tick(256);
        $stdout .= stream_get_contents($pipes[1]);
        $stderr .= stream_get_contents($pipes[2]);

        if ($transitionOnly && !$transitionCommitted) {
            $store = $server->world()->nativeStore();
            worldSyncExpect($store !== null, 'transition verification lost native store');

            $enteringPinned = true;
            $leavingReleased = true;
            for ($chunkZ = 6; $chunkZ <= 10; ++$chunkZ) {
                try {
                    $enteringPinned = $enteringPinned
                        && $store->chunkPinCount(new ChunkPos(11, $chunkZ)) > 0;
                } catch (Throwable) {
                    $enteringPinned = false;
                }
                try {
                    $leavingReleased = $leavingReleased
                        && $store->chunkPinCount(new ChunkPos(6, $chunkZ)) === 0;
                } catch (Throwable) {
                    // A released leaving chunk may be evicted immediately.
                }
            }

            if ($enteringPinned && $leavingReleased) {
                worldSyncExpect(
                    $store->chunkPinCount(new ChunkPos(8, 8)) > 0,
                    'committed transition lost an overlapping view pin',
                );
                $transitionCommitted = true;
            }
        }

        if ($radiusCycle && !$radiusCycleCommitted) {
            $store = $server->world()->nativeStore();
            worldSyncExpect($store !== null, 'radius-cycle verification lost native store');

            $innerPinned = true;
            for ($chunkX = 7; $chunkX <= 9; ++$chunkX) {
                for ($chunkZ = 7; $chunkZ <= 9; ++$chunkZ) {
                    $innerPinned = $innerPinned
                        && worldSyncPinCountOrZero($store, new ChunkPos($chunkX, $chunkZ)) > 0;
                }
            }

            $outerReleased = true;
            for ($chunkX = 5; $chunkX <= 11; ++$chunkX) {
                for ($chunkZ = 5; $chunkZ <= 11; ++$chunkZ) {
                    if ((7 <= $chunkX && $chunkX <= 9) && (7 <= $chunkZ && $chunkZ <= 9)) {
                        continue;
                    }
                    $outerReleased = $outerReleased
                        && worldSyncPinCountOrZero($store, new ChunkPos($chunkX, $chunkZ)) === 0;
                }
            }

            $radiusCycleCommitted = $innerPinned && $outerReleased;
        }

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
    if (!$radiusCycle) {
        worldSyncExpect($movementHandled, 'world-sync MovePlayer never reached post-spawn gameplay handling');
    }

    if ($radiusCycle) {
        worldSyncExpect($radiusCycleCommitted, 'radius-cycle view was never committed down to radius 1');
        worldSyncExpect(
            str_contains($stdout, 'world-sync-client: radius-cycle=verified grow=24 shrink=1'),
            "world-sync client did not observe radius grow/shrink\nstdout={$stdout}\nstderr={$stderr}",
        );
    } elseif ($transitionOnly) {
        worldSyncExpect($transitionCommitted, 'prepared chunk view was never committed');
        worldSyncExpect(
            str_contains($stdout, 'world-sync-client: transition=verified entering=5'),
            "world-sync client did not observe five entering chunks\nstdout={$stdout}\nstderr={$stderr}",
        );
    } else {
        worldSyncExpect($mutated, 'world-sync mutation was not applied');
        worldSyncExpect(
            str_contains($stdout, 'world-sync-client: update=verified'),
            "world-sync client did not observe UpdateBlock\nstdout={$stdout}\nstderr={$stderr}",
        );
    }
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
    worldSyncPinCountOrZero($store, new ChunkPos(8, 8)) === 0,
    'session shutdown did not release streamed chunk pins',
);
worldSyncExpect(
    worldSyncPinCountOrZero($store, new ChunkPos(11, 8)) === 0,
    'session shutdown did not release prepared entering chunk pins',
);

fwrite(STDOUT, "world-sync-smoke: passed\n");
