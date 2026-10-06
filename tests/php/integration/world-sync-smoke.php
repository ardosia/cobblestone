<?php

declare(strict_types=1);

require dirname(__DIR__) . '/bootstrap.php';

$transitionOnly = in_array('--transition-only', $argv, true);
$radiusCycle = in_array('--radius-cycle', $argv, true);
$wideInitial = in_array('--wide-initial', $argv, true);
$streamTorture = in_array('--stream-torture', $argv, true);

use Cobblestone\Native\Session\Packet;
use Cobblestone\Server\Server;
use Cobblestone\Config\ServerConfig;
use Cobblestone\Server\ServerState;
use Cobblestone\Session\Event\SessionSpawned;
use Cobblestone\World\BlockPos;
use Cobblestone\World\BlockStateId;
use Cobblestone\World\BlockType;
use Cobblestone\World\ChunkPos;
use Cobblestone\Native\World as NativeWorld;

function worldSyncExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

if ($wideInitial) {
    worldSyncExpect((new ServerConfig())->initialChunkRadius === 3, 'server default chunk radius is not 3');
}

function worldSyncPinCountOrZero(NativeWorld $store, ChunkPos $position): int
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
$server = Server::create(
    new ServerConfig(
        $bind,
        4,
        'Cobblestone World Sync Test',
        initialChunkRadius: $radiusCycle || $wideInitial ? 3 : 2,
    ),
    packetHandler: static function (Packet $packet) use (&$movementHandled, &$server, $transitionOnly): void {
        if ($packet->packetId !== 0x10) {
            return;
        }

        if ($transitionOnly) {
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
        }
        $movementHandled = true;
    },
);
$spawned = false;
$mutated = false;
$server->on(
    SessionSpawned::class,
    static function (SessionSpawned $event) use ($server, $transitionOnly, $radiusCycle, $wideInitial, $streamTorture, &$spawned, &$mutated): void {
        $spawned = true;
        worldSyncExpect($event->chunksSent === ($wideInitial ? 49 : 25), 'world-sync initial view chunk count mismatch');
        $store = $server->world()->nativeStore();
        worldSyncExpect($store !== null, 'world-sync server did not use native world storage');
        worldSyncExpect(
            $store->chunkPinCount(new ChunkPos(8, 8)) > 0,
            'spawned client view did not pin its streamed center chunk',
        );
        if ($transitionOnly || $radiusCycle || $wideInitial || $streamTorture) {
            return;
        }

        $previous = $server->world()->setBlockStateId(
            new BlockPos(128, 5, 128),
            BlockStateId::encode(BlockType::Stone),
        );
        worldSyncExpect(
            $previous === BlockStateId::encode(BlockType::Air),
            'world-sync mutation expected air before stone',
        );
        $mutated = true;
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
    $streamTorture
        ? '--stream-torture'
        : ($wideInitial ? '--spawn-only' : ($radiusCycle ? '--radius-cycle' : ($transitionOnly ? '--transition-only' : '--move-after-update'))),
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
    $server->stop('world-sync-client-start-failed');
    $server->shutdown();
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
$streamTortureCommitted = false;

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

        if (
            $streamTorture
            && !$streamTortureCommitted
            && str_contains($stdout, 'world-sync-client: stream-torture returned=verified')
        ) {
            $store = $server->world()->nativeStore();
            worldSyncExpect($store !== null, 'stream-torture verification lost native store');

            $finalPinned = true;
            for ($chunkX = 6; $chunkX <= 10; ++$chunkX) {
                for ($chunkZ = 6; $chunkZ <= 10; ++$chunkZ) {
                    $finalPinned = $finalPinned
                        && worldSyncPinCountOrZero($store, new ChunkPos($chunkX, $chunkZ)) > 0;
                }
            }

            $diagonalReleased = true;
            for ($chunkX = 11; $chunkX <= 15; ++$chunkX) {
                for ($chunkZ = 8; $chunkZ <= 12; ++$chunkZ) {
                    $diagonalReleased = $diagonalReleased
                        && worldSyncPinCountOrZero($store, new ChunkPos($chunkX, $chunkZ)) === 0;
                }
            }

            $streamTortureCommitted = $finalPinned && $diagonalReleased;
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
    if (!$radiusCycle && !$wideInitial) {
        worldSyncExpect($movementHandled, 'world-sync MovePlayer never reached post-spawn gameplay handling');
    }

    if ($wideInitial) {
        worldSyncExpect(
            str_contains($stdout, 'world-sync-client: initial-radius=verified radius=3 chunks=49'),
            "world-sync client did not observe all 49 initial chunks\nstdout={$stdout}\nstderr={$stderr}",
        );
    } elseif ($streamTorture) {
        worldSyncExpect($streamTortureCommitted, 'stream-torture did not return to the original view cleanly');
        foreach ([
            'world-sync-client: stream-torture rapid=verified',
            'world-sync-client: stream-torture diagonal=verified',
            'world-sync-client: stream-torture returned=verified',
            'world-sync-client: stream-torture=verified',
        ] as $marker) {
            worldSyncExpect(
                str_contains($stdout, $marker),
                "world-sync client missed stream-torture marker {$marker}\nstdout={$stdout}\nstderr={$stderr}",
            );
        }
    } elseif ($radiusCycle) {
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
        $server->stop('world-sync-smoke');
        $server->shutdown();
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
