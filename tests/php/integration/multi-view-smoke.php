<?php

declare(strict_types=1);

require dirname(__DIR__) . '/bootstrap.php';
require_once __DIR__ . '/ClientOutput.php';

use Cobblestone\Server\Server;
use Cobblestone\Config\ServerConfig;
use Cobblestone\Server\ServerState;
use Cobblestone\Session\Event\SessionDisconnected;
use Cobblestone\Session\Event\SessionSpawned;
use Cobblestone\World\ChunkPos;
use Cobblestone\Native\World as NativeWorld;

function multiViewExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

function multiViewPinCountOrZero(NativeWorld $store, ChunkPos $position): int
{
    try {
        return $store->chunkPinCount($position);
    } catch (Throwable) {
        return 0;
    }
}

$probe = stream_socket_server('udp://127.0.0.1:0', $errorCode, $errorMessage, STREAM_SERVER_BIND);
if ($probe === false) {
    throw new RuntimeException("failed to allocate loopback UDP port: {$errorCode} {$errorMessage}");
}
$bind = stream_socket_get_name($probe, false);
fclose($probe);
multiViewExpect(is_string($bind) && $bind !== '', 'failed to resolve loopback UDP address');

$server = Server::create(new ServerConfig($bind, 4, 'Cobblestone Multi View Test'));
$spawned = 0;
/** @var list<int> $sessionIds */
$sessionIds = [];
$disconnected = 0;
$server->on(
    SessionSpawned::class,
    static function (SessionSpawned $event) use (&$spawned, &$sessionIds): void {
        ++$spawned;
        $sessionIds[] = $event->sessionId;
    },
);
$server->on(
    SessionDisconnected::class,
    static function () use (&$disconnected): void {
        ++$disconnected;
    },
);

$server->start();

$root = dirname(__DIR__, 3);
$clients = [];
foreach (['east' => '--hold-east', 'west' => '--hold-west'] as $name => $mode) {
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
        $mode,
    ];
    $output = new ClientOutput();
    $process = proc_open(
        $command,
        $output->descriptors(),
        $pipes,
        $root,
    );
    if (!is_resource($process)) {
        throw new RuntimeException("failed to start {$name} loopback client");
    }
    fclose($pipes[0]);
    $clients[$name] = [
        'process' => $process,
        'output' => $output,
        'stdout' => '',
        'stderr' => '',
        'exit' => null,
    ];
}

$sharedVerified = false;
$disconnectRequested = false;
$deadline = hrtime(true) + 20_000_000_000;

try {
    while (hrtime(true) < $deadline) {
        $server->tick(256);

        foreach ($clients as $name => &$client) {
            $client['stdout'] .= $client['output']->readStdout();
            $client['stderr'] .= $client['output']->readStderr();
            if ($client['exit'] !== null) {
                continue;
            }

            $status = proc_get_status($client['process']);
            if (!$status['running']) {
                $client['exit'] = $status['exitcode'];
            }
        }
        unset($client);

        if (
            !$sharedVerified
            && str_contains($clients['east']['stdout'], 'world-sync-client: shared-view east=ready')
            && str_contains($clients['west']['stdout'], 'world-sync-client: shared-view west=ready')
        ) {
            $store = $server->world()->nativeStore();
            multiViewExpect($store !== null, 'multi-view server lost native world store');

            for ($z = 6; $z <= 10; ++$z) {
                multiViewExpect(
                    multiViewPinCountOrZero($store, new ChunkPos(8, $z)) === 2,
                    "overlap chunk 8:{$z} did not have two viewer pins",
                );
                multiViewExpect(
                    multiViewPinCountOrZero($store, new ChunkPos(12, $z)) === 1,
                    "east-only chunk 12:{$z} did not have one viewer pin",
                );
                multiViewExpect(
                    multiViewPinCountOrZero($store, new ChunkPos(4, $z)) === 1,
                    "west-only chunk 4:{$z} did not have one viewer pin",
                );
            }
            $sharedVerified = true;
        }

        if (
            $sharedVerified
            && !$disconnectRequested
            && $clients['east']['exit'] !== null
            && $clients['west']['exit'] !== null
            && count($sessionIds) === 2
        ) {
            $runtime = testNativeSessions($server);
            foreach ($sessionIds as $sessionId) {
                $runtime->disconnect($sessionId);
            }
            $disconnectRequested = true;
        }

        if ($disconnectRequested && $disconnected === 2) {
            $store = $server->world()->nativeStore();
            multiViewExpect($store !== null, 'multi-view native store disappeared during cleanup');
            $pinsCleared = true;
            for ($x = 4; $x <= 12; ++$x) {
                for ($z = 6; $z <= 10; ++$z) {
                    $pinsCleared = $pinsCleared
                        && multiViewPinCountOrZero($store, new ChunkPos($x, $z)) === 0;
                }
            }
            if ($pinsCleared) {
                break;
            }
        }

        usleep(1_000);
    }

    $diagnostic = sprintf(
        " east_exit=%s west_exit=%s east_stdout=%s east_stderr=%s west_stdout=%s west_stderr=%s",
        var_export($clients['east']['exit'], true),
        var_export($clients['west']['exit'], true),
        trim($clients['east']['stdout']),
        trim($clients['east']['stderr']),
        trim($clients['west']['stdout']),
        trim($clients['west']['stderr']),
    );
    multiViewExpect($spawned === 2, "expected two spawned clients, got {$spawned}{$diagnostic}");
    multiViewExpect($sharedVerified, "overlapping streamed residency was never observed{$diagnostic}");
    multiViewExpect($disconnectRequested, "owner disconnect was never requested{$diagnostic}");
    multiViewExpect($disconnected === 2, "expected two disconnected clients, got {$disconnected}{$diagnostic}");

    foreach ($clients as $name => &$client) {
        $client['stdout'] .= $client['output']->readStdout();
        $client['stderr'] .= $client['output']->readStderr();
        multiViewExpect(
            $client['exit'] === 0,
            "{$name} client failed: {$client['stderr']}",
        );
    }
    unset($client);

    $store = $server->world()->nativeStore();
    multiViewExpect($store !== null, 'multi-view native store disappeared');
    for ($x = 4; $x <= 12; ++$x) {
        for ($z = 6; $z <= 10; ++$z) {
            multiViewExpect(
                multiViewPinCountOrZero($store, new ChunkPos($x, $z)) === 0,
                "chunk {$x}:{$z} leaked a viewer pin after both disconnects",
            );
        }
    }
} finally {
    foreach ($clients as &$client) {
        if (is_resource($client['process'])) {
            proc_close($client['process']);
        }
        $client['output']->close();
    }
    unset($client);

    if ($server->state() === ServerState::Running) {
        $server->stop('multi-view-smoke');
        $server->shutdown();
    }
}

fwrite(STDOUT, "multi-view-smoke: passed\n");
