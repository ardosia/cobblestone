<?php

declare(strict_types=1);

require __DIR__ . '/bootstrap.php';

use Cobblestone\Native\Session\Packet;
use Cobblestone\Native\Session\Runtime;
use Cobblestone\Server\Server;
use Cobblestone\Server\ServerState;
use Cobblestone\Server\WorldFactory;
use Cobblestone\Session\ChunkViewPreparation;
use Cobblestone\Session\Event\SessionDisconnected;
use Cobblestone\Session\Event\SessionSpawned;
use Cobblestone\Session\SessionGameplay;
use Cobblestone\World\BiomeId;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\NativeWorldStore;
use Cobblestone\World\ResidentChunkHandle;

function pendingDisconnectExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

function pendingDisconnectPinCountOrZero(NativeWorldStore $store, ChunkPos $position): int
{
    try {
        return $store->chunkPinCount($position);
    } catch (Throwable) {
        return 0;
    }
}

function pendingDisconnectRemoveTree(string $root): void
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
    . '/cobblestone-pending-disconnect-'
    . getmypid()
    . '-'
    . bin2hex(random_bytes(4));

$world = WorldFactory::persistentFlat(
    $storageRoot,
    'Pending Disconnect',
    5150,
    '2;7,2x3,2;1;',
    saveWorkers: 1,
    loadWorkers: 1,
);
$store = $world->nativeStore();
pendingDisconnectExpect($store !== null, 'pending-disconnect test requires native storage');

$center = $world->spawn()->chunk();
$partialPositions = [
    new ChunkPos($center->x + 3, $center->z - 2),
    new ChunkPos($center->x + 3, $center->z - 1),
];

/** @var list<ResidentChunkHandle> $baselineHandles */
$baselineHandles = [];
foreach ($partialPositions as $position) {
    $store->ensureChunk($position, new BiomeId(1));
    $world->adoptNativeChunk($position);
    $handle = $world->residentChunk($position, false);
    pendingDisconnectExpect($handle !== null, 'failed to pin pre-resident entering chunk');
    $baselineHandles[] = $handle;
}

$probe = stream_socket_server('udp://127.0.0.1:0', $errorCode, $errorMessage, STREAM_SERVER_BIND);
if ($probe === false) {
    throw new RuntimeException("failed to allocate loopback UDP port: {$errorCode} {$errorMessage}");
}
$bind = stream_socket_get_name($probe, false);
fclose($probe);
pendingDisconnectExpect(is_string($bind) && $bind !== '', 'failed to resolve loopback UDP address');

$server = null;
$pendingObserved = false;
$cleanupTriggered = false;
$spawned = false;
$disconnected = false;

$server = Server::create(
    $bind,
    2,
    'Cobblestone Pending Disconnect Test',
    static function (Packet $packet) use (
        &$server,
        $store,
        $center,
        $partialPositions,
        &$baselineHandles,
        &$pendingObserved,
        &$cleanupTriggered,
    ): void {
        if ($packet->packetId !== 0x10 || $cleanupTriggered) {
            return;
        }

        pendingDisconnectExpect($server instanceof Server, 'pending-disconnect server unavailable');

        $gameplayProperty = new ReflectionProperty(Server::class, 'gameplay');
        $gameplay = $gameplayProperty->getValue($server);
        pendingDisconnectExpect($gameplay instanceof SessionGameplay, 'gameplay state unavailable');

        $preparationsProperty = new ReflectionProperty(SessionGameplay::class, 'preparations');
        $preparations = $preparationsProperty->getValue($gameplay);
        $preparation = $preparations[$packet->sessionId] ?? null;
        pendingDisconnectExpect(
            $preparation instanceof ChunkViewPreparation,
            'MovePlayer did not create a chunk view preparation',
        );
        pendingDisconnectExpect(
            !$preparation->prepared(),
            'transition unexpectedly completed before pending-disconnect assertion',
        );

        foreach ($partialPositions as $position) {
            pendingDisconnectExpect(
                $store->chunkPinCount($position) === 2,
                "pre-resident entering chunk {$position->key()} did not hold baseline + preparation pins",
            );
        }
        pendingDisconnectExpect(
            $store->chunkPinCount($center) > 0,
            'pending transition released the old active center',
        );
        $pendingObserved = true;

        foreach ($baselineHandles as $handle) {
            $handle->release();
        }
        $baselineHandles = [];
        foreach ($partialPositions as $position) {
            pendingDisconnectExpect(
                $store->chunkPinCount($position) === 1,
                "preparation pin disappeared before disconnect cleanup for {$position->key()}",
            );
        }

        $gameplay->disconnected($packet->sessionId);
        foreach ($partialPositions as $position) {
            pendingDisconnectExpect(
                pendingDisconnectPinCountOrZero($store, $position) === 0,
                "disconnect cleanup leaked temporary entering pin for {$position->key()}",
            );
        }
        pendingDisconnectExpect(
            $store->chunkPinCount($center) > 0,
            'PHP pending cleanup released the native active view too early',
        );

        $sessionsProperty = new ReflectionProperty(Server::class, 'sessions');
        $runtime = $sessionsProperty->getValue($server);
        pendingDisconnectExpect($runtime instanceof Runtime, 'session runtime unavailable');
        $runtime->disconnect($packet->sessionId);
        $cleanupTriggered = true;
    },
    null,
    $world,
);

$server->on(
    SessionSpawned::class,
    static function () use (&$spawned): void {
        $spawned = true;
    },
);
$server->on(
    SessionDisconnected::class,
    static function () use (&$disconnected): void {
        $disconnected = true;
    },
);

$server->start();

$root = dirname(__DIR__, 2);
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
        '--pending-disconnect',
    ],
    [0 => ['pipe', 'r'], 1 => ['pipe', 'w'], 2 => ['pipe', 'w']],
    $pipes,
    $root,
);
pendingDisconnectExpect(is_resource($process), 'failed to start pending-disconnect client');
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
        if (!$status['running'] && $exitCode === null) {
            $exitCode = $status['exitcode'];
        }

        if ($pendingObserved && $cleanupTriggered && $disconnected && $exitCode !== null) {
            break;
        }
        usleep(1_000);
    }

    $stdout .= stream_get_contents($pipes[1]);
    $stderr .= stream_get_contents($pipes[2]);

    pendingDisconnectExpect($spawned, 'pending-disconnect client never spawned');
    pendingDisconnectExpect($pendingObserved, 'pending transition was never observed');
    pendingDisconnectExpect($cleanupTriggered, 'disconnect cleanup was never triggered');
    pendingDisconnectExpect($disconnected, 'native session disconnect was never observed');
    pendingDisconnectExpect($exitCode === 0, "pending-disconnect client failed: {$stderr}");
    pendingDisconnectExpect(
        str_contains($stdout, 'world-sync-client: pending-disconnect movement=sent'),
        "pending-disconnect client did not send movement\nstdout={$stdout}\nstderr={$stderr}",
    );

    for ($x = $center->x - 2; $x <= $center->x + 2; ++$x) {
        for ($z = $center->z - 2; $z <= $center->z + 2; ++$z) {
            pendingDisconnectExpect(
                pendingDisconnectPinCountOrZero($store, new ChunkPos($x, $z)) === 0,
                "old active view chunk {$x}:{$z} leaked a native pin after disconnect",
            );
        }
    }
    foreach ($partialPositions as $position) {
        pendingDisconnectExpect(
            pendingDisconnectPinCountOrZero($store, $position) === 0,
            "temporary entering chunk {$position->key()} leaked after disconnect",
        );
    }
} finally {
    foreach ($baselineHandles as $handle) {
        try {
            $handle->release();
        } catch (Throwable) {
        }
    }
    $baselineHandles = [];

    foreach ([1, 2] as $pipe) {
        if (isset($pipes[$pipe]) && is_resource($pipes[$pipe])) {
            fclose($pipes[$pipe]);
        }
    }
    if (is_resource($process)) {
        proc_close($process);
    }
    if ($server instanceof Server && $server->state() === ServerState::Running) {
        $server->stop('pending-disconnect-smoke');
        $server->shutdown();
    }
    try {
        $store->destroy();
    } catch (Throwable) {
    }
    pendingDisconnectRemoveTree($storageRoot);
}

fwrite(STDOUT, "pending-disconnect-smoke: passed\n");
