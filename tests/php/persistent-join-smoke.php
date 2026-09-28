<?php

declare(strict_types=1);

require __DIR__ . '/bootstrap.php';

use Cobblestone\Server\Server;
use Cobblestone\Server\ServerState;
use Cobblestone\Server\WorldFactory;
use Cobblestone\Session\Event\SessionSpawned;
use Cobblestone\World\BlockPos;
use Cobblestone\World\BlockStateId;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\NativeChunkLoadStatus;
use Cobblestone\World\NativeWorldStore;

function persistentJoinExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

function persistentJoinRemoveTree(string $root): void
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

function persistentJoinPositions(ChunkPos $center, int $radius): array
{
    $positions = [];
    for ($x = $center->x - $radius; $x <= $center->x + $radius; ++$x) {
        for ($z = $center->z - $radius; $z <= $center->z + $radius; ++$z) {
            $positions[] = new ChunkPos($x, $z);
        }
    }

    return $positions;
}

$storageRoot = sys_get_temp_dir()
    . '/cobblestone-persistent-join-'
    . getmypid()
    . '-'
    . bin2hex(random_bytes(4));
$seed = 424242;
$preset = '2;7,2x3,2;1;';

$seedWorld = WorldFactory::persistentFlat(
    $storageRoot,
    'Persistent Join',
    $seed,
    $preset,
    saveWorkers: 2,
    loadWorkers: 2,
);
$seedStore = $seedWorld->nativeStore();
persistentJoinExpect($seedStore !== null, 'persistent join requires native world storage');

$center = $seedWorld->spawn()->chunk();
$positions = persistentJoinPositions($center, 2);
$projection = NativeWorldStore::encodeStorageLoadBatch($positions);

try {
    for ($attempt = 0; $attempt < 1000; ++$attempt) {
        $statuses = $seedStore->prepareStorageLoadBatch($projection);
        if ($statuses === str_repeat(chr(NativeChunkLoadStatus::Missing->value), count($positions))) {
            break;
        }
        usleep(1_000);
    }
    persistentJoinExpect(
        $statuses === str_repeat(chr(NativeChunkLoadStatus::Missing->value), count($positions)),
        'new persistent world did not resolve initial chunks as durable misses',
    );

    foreach ($positions as $position) {
        $seedWorld->chunk($position, true);
    }

    $allClean = false;
    for ($attempt = 0; $attempt < 2000; ++$attempt) {
        $state = $seedStore->storageTick(64);
        $allClean = $state['in_flight'] === 0;
        foreach ($positions as $position) {
            $chunk = $seedWorld->chunk($position, false);
            if ($chunk === null || $chunk->isDirty()) {
                $allClean = false;
                break;
            }
        }
        if ($allClean) {
            break;
        }
        usleep(1_000);
    }
    persistentJoinExpect($allClean, 'seed persistent chunks did not reach durable clean state');
} finally {
    $seedStore->destroy();
}
unset($seedWorld, $seedStore);

$world = WorldFactory::persistentFlat(
    $storageRoot,
    'Ignored Creation Name',
    -999,
    '2;1;1;',
    saveWorkers: 2,
    loadWorkers: 2,
);
$store = $world->nativeStore();
persistentJoinExpect($store !== null, 'reopened persistent join world lacks native store');
persistentJoinExpect($world->name() === 'Persistent Join', 'stored world name did not override creation default');
persistentJoinExpect($world->seed() === $seed, 'stored world seed did not override creation default');
persistentJoinExpect(
    ($world->generator()->settings()['preset'] ?? null) === $preset,
    'stored Flat preset did not override creation default',
);
persistentJoinExpect(
    $world->spawn()->x === 128 && $world->spawn()->y === 4 && $world->spawn()->z === 128,
    'stored world spawn did not override conflicting creation preset',
);

$probe = stream_socket_server(
    'udp://127.0.0.1:0',
    $errorCode,
    $errorMessage,
    STREAM_SERVER_BIND,
);
if ($probe === false) {
    $store->destroy();
    persistentJoinRemoveTree($storageRoot);
    throw new RuntimeException("failed to allocate loopback UDP port: {$errorCode} {$errorMessage}");
}
$bind = stream_socket_get_name($probe, false);
fclose($probe);
persistentJoinExpect(is_string($bind) && $bind !== '', 'failed to resolve loopback UDP address');

$server = Server::start(
    $bind,
    4,
    'Cobblestone Persistent Join Test',
    world: $world,
);
$spawned = false;
$mutated = false;
$server->events()->listen(
    SessionSpawned::class,
    static function (SessionSpawned $event) use ($server, $center, &$spawned, &$mutated): void {
        $spawned = true;
        persistentJoinExpect($event->chunksSent === 25, 'persistent join sent wrong initial chunk count');
        persistentJoinExpect(
            $server->world()->chunks()->count() === 25,
            'persistent join did not adopt loaded chunks into PHP residency',
        );

        $chunk = $server->world()->chunk($center, false);
        persistentJoinExpect($chunk !== null, 'persistent center chunk facade is missing');
        persistentJoinExpect(
            $chunk->blockStateId(0, 0, 0) === BlockStateId::fromLegacy(7),
            'persistent center chunk did not retain bedrock from disk',
        );
        persistentJoinExpect(!$chunk->isDirty(), 'loaded persistent center chunk entered residency dirty');

        $previous = $server->world()->setBlockStateId(
            new BlockPos(128, 5, 128),
            BlockStateId::fromLegacy(1),
        );
        persistentJoinExpect(
            $previous === BlockStateId::fromLegacy(0),
            'persistent join mutation expected air before stone',
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
    $server->requestStop('persistent-join-client-start-failed');
    $server->stop();
    $store->destroy();
    persistentJoinRemoveTree($storageRoot);
    throw new RuntimeException('failed to start persistent-join loopback client');
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
            "persistent-join loopback client timed out\nstdout={$stdout}\nstderr={$stderr}",
        );
    }

    $stdout .= stream_get_contents($pipes[1]);
    $stderr .= stream_get_contents($pipes[2]);

    persistentJoinExpect($exitCode === 0, "persistent-join client failed: {$stderr}");
    persistentJoinExpect($spawned, 'persistent-join session never reached spawned state');
    persistentJoinExpect($mutated, 'persistent-join mutation was not applied');
    persistentJoinExpect(
        str_contains($stdout, 'world-sync-client: update=verified'),
        "persistent-join client did not observe UpdateBlock\nstdout={$stdout}\nstderr={$stderr}",
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
        $server->requestStop('persistent-join-smoke');
        $server->stop();
    }

    persistentJoinExpect(
        $store->chunkPinCount($center) === 0,
        'persistent session shutdown did not release streamed chunk pins',
    );
    $store->destroy();
    persistentJoinRemoveTree($storageRoot);
}

fwrite(STDOUT, "persistent-join-smoke: passed\n");
