<?php

declare(strict_types=1);

require dirname(__DIR__) . '/bootstrap.php';

use Cobblestone\Config\ServerConfig;
use Cobblestone\Native\Session\Packet;
use Cobblestone\Native\World as NativeWorld;
use Cobblestone\Server\Server;
use Cobblestone\Server\ServerState;
use Cobblestone\Session\Event\SessionDisconnected;
use Cobblestone\Session\Event\SessionSpawned;
use Cobblestone\World\ChunkPos;

function backpressureViewExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

function backpressureViewPinCountOrZero(NativeWorld $store, ChunkPos $position): int
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
backpressureViewExpect(is_string($bind) && $bind !== '', 'failed to resolve loopback UDP address');

$server = null;
$spawnedSessionId = null;
$backpressureInjected = false;
$retryCommitted = false;
$disconnected = false;

$server = Server::create(
    new ServerConfig($bind, 2, 'Cobblestone Backpressure View Test'),
    packetHandler: static function (Packet $packet) use (&$server, &$backpressureInjected): void {
        if ($packet->packetId !== 0x10 || $backpressureInjected) {
            return;
        }
        backpressureViewExpect($server instanceof Server, 'backpressure test server unavailable');
        $sessions = testNativeSessions($server);
        $payload = str_repeat("\0", 16 * 1024);
        for ($attempt = 0; $attempt < 4096; ++$attempt) {
            try {
                $sessions->send($packet->sessionId, 0x7f, $payload);
            } catch (Throwable $error) {
                if (!str_contains($error->getMessage(), 'command queue is full')) {
                    throw $error;
                }
                $backpressureInjected = true;
                return;
            }
        }
        throw new RuntimeException('native session command queue never surfaced backpressure');
    },
);

$server->on(
    SessionSpawned::class,
    static function (SessionSpawned $event) use (&$spawnedSessionId): void {
        $spawnedSessionId = $event->sessionId;
    },
);
$server->on(
    SessionDisconnected::class,
    static function () use (&$disconnected): void {
        $disconnected = true;
    },
);

$server->start();
$root = dirname(__DIR__, 3);
$process = proc_open(
    [
        'cargo',
        'run',
        '--quiet',
        '-p',
        'cobblestone-client-bootstrap',
        '--bin',
        'world-sync-client',
        '--',
        $bind,
        '--transition-only',
    ],
    [0 => ['pipe', 'r'], 1 => ['pipe', 'w'], 2 => ['pipe', 'w']],
    $pipes,
    $root,
);
backpressureViewExpect(is_resource($process), 'failed to start backpressure loopback client');
fclose($pipes[0]);
stream_set_blocking($pipes[1], false);
stream_set_blocking($pipes[2], false);

$stdout = '';
$stderr = '';
$exitCode = null;
$disconnectRequested = false;
$deadline = hrtime(true) + 30_000_000_000;

try {
    while (hrtime(true) < $deadline) {
        $server->tick(256);
        $stdout .= stream_get_contents($pipes[1]);
        $stderr .= stream_get_contents($pipes[2]);

        if (
            $backpressureInjected
            && !$retryCommitted
            && str_contains($stdout, 'world-sync-client: transition=verified entering=5')
        ) {
            $store = $server->world()->nativeStore();
            backpressureViewExpect($store !== null, 'native store unavailable after retry');
            backpressureViewExpect(
                backpressureViewPinCountOrZero($store, new ChunkPos(6, 8)) === 0,
                'successful retry did not retire the old leaving edge',
            );
            backpressureViewExpect(
                backpressureViewPinCountOrZero($store, new ChunkPos(11, 8)) > 0,
                'successful retry did not transfer entrant ownership to the active view',
            );
            $retryCommitted = true;
        }

        $status = proc_get_status($process);
        if (!$status['running'] && $exitCode === null) {
            $exitCode = $status['exitcode'];
        }
        if (
            $retryCommitted
            && !$disconnectRequested
            && $exitCode !== null
            && is_int($spawnedSessionId)
        ) {
            testNativeSessions($server)->disconnect($spawnedSessionId);
            $disconnectRequested = true;
        }
        if ($retryCommitted && $disconnectRequested && $disconnected) {
            break;
        }
        usleep(1_000);
    }

    $stdout .= stream_get_contents($pipes[1]);
    $stderr .= stream_get_contents($pipes[2]);
    backpressureViewExpect(is_int($spawnedSessionId), 'backpressure client never spawned');
    backpressureViewExpect($backpressureInjected, 'native session queue never reached backpressure');
    backpressureViewExpect($retryCommitted, 'retry never committed the prepared transition');
    backpressureViewExpect($exitCode === 0, "backpressure client failed: {$stderr}");
    backpressureViewExpect($disconnected, 'backpressure session disconnect was never observed');
} finally {
    foreach ([1, 2] as $pipe) {
        if (isset($pipes[$pipe]) && is_resource($pipes[$pipe])) {
            fclose($pipes[$pipe]);
        }
    }
    if (is_resource($process)) {
        proc_close($process);
    }
    if ($server instanceof Server && $server->state() === ServerState::Running) {
        $server->stop('backpressure-view-smoke');
        $server->shutdown();
    }
}

fwrite(STDOUT, "backpressure-view-smoke: passed\n");
