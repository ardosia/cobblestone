<?php

declare(strict_types=1);

require __DIR__ . '/bootstrap.php';

use Cobblestone\Server\Server;
use Cobblestone\Server\ServerState;
use Cobblestone\Server\WorldFactory;
use Cobblestone\Session\Event\SessionDisconnected;
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
$evictAfterDisconnectPosition = new ChunkPos($center->x - 2, $center->z - 2);
$fiberPersistedPosition = new ChunkPos($center->x + 4, $center->z);
$fiberMissingPosition = new ChunkPos($center->x + 5, $center->z);
$positions = [
    ...persistentJoinPositions($center, 2),
    $fiberPersistedPosition,
];
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
$disconnected = false;
$mutated = false;
$stopFlushDirty = false;
$server->events()->listen(
    SessionDisconnected::class,
    static function (SessionDisconnected $event) use (&$disconnected): void {
        $disconnected = true;
    },
);
$server->events()->listen(
    SessionSpawned::class,
    static function (SessionSpawned $event) use (
        $server,
        $store,
        $center,
        $evictAfterDisconnectPosition,
        &$spawned,
        &$mutated,
    ): void {
        $spawned = true;
        persistentJoinExpect($event->chunksSent === 25, 'persistent join sent wrong initial chunk count');
        persistentJoinExpect(
            $server->world()->chunks()->count() === 25,
            'persistent join did not adopt loaded chunks into PHP residency',
        );
        persistentJoinExpect(
            $store->chunkPinCount($evictAfterDisconnectPosition) === 1,
            'spawned session did not pin its initial chunk view',
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

    for ($attempt = 0; $attempt < 1000; ++$attempt) {
        if (
            $disconnected
            && $server->world()->chunk($evictAfterDisconnectPosition, false) === null
        ) {
            break;
        }
        $server->tick(1);
        usleep(1_000);
    }
    persistentJoinExpect($disconnected, 'persistent session disconnect was not observed');
    persistentJoinExpect(
        $server->world()->chunk($evictAfterDisconnectPosition, false) === null,
        'clean initial-view chunk was not evicted after the session released its pin',
    );

    $loadedHandle = null;
    $server->scheduler()->spawn(
        static function () use ($server, $fiberPersistedPosition, &$loadedHandle): void {
            $loadedHandle = $server->awaitResidentChunk($fiberPersistedPosition);
        },
    );
    persistentJoinExpect(
        $loadedHandle === null,
        'persisted gameplay chunk acquisition did not suspend while storage was unresolved',
    );
    for ($attempt = 0; $attempt < 1000 && $loadedHandle === null; ++$attempt) {
        $server->tick(1);
        usleep(1_000);
    }
    persistentJoinExpect($loadedHandle !== null, 'persisted gameplay chunk acquisition never resumed');
    persistentJoinExpect(
        $loadedHandle->position()->key() === $fiberPersistedPosition->key(),
        'persisted gameplay chunk acquisition resumed with the wrong chunk',
    );
    persistentJoinExpect(
        $store->chunkPinCount($fiberPersistedPosition) === 1,
        'Fiber gameplay acquisition did not pin the loaded chunk',
    );
    persistentJoinExpect(
        !$store->chunkDirty($fiberPersistedPosition),
        'Fiber gameplay acquisition rewrote a persisted chunk instead of adopting it',
    );
    $loadedHandle->release();
    persistentJoinExpect(
        $store->chunkPinCount($fiberPersistedPosition) === 0,
        'Fiber gameplay acquisition did not release the loaded chunk pin',
    );

    $generatedHandle = null;
    $server->scheduler()->spawn(
        static function () use ($server, $fiberMissingPosition, &$generatedHandle): void {
            $generatedHandle = $server->awaitResidentChunk($fiberMissingPosition);
        },
    );
    persistentJoinExpect(
        $generatedHandle === null,
        'missing gameplay chunk acquisition did not suspend before durable miss resolution',
    );
    for ($attempt = 0; $attempt < 1000 && $generatedHandle === null; ++$attempt) {
        $server->tick(1);
        usleep(1_000);
    }
    persistentJoinExpect($generatedHandle !== null, 'missing gameplay chunk acquisition never resumed');
    persistentJoinExpect(
        $generatedHandle->position()->key() === $fiberMissingPosition->key(),
        'missing gameplay chunk acquisition generated the wrong chunk',
    );
    persistentJoinExpect(
        $store->chunkDirty($fiberMissingPosition),
        'durably missing gameplay chunk did not enter residency dirty after generation',
    );
    persistentJoinExpect(
        $store->chunkPinCount($fiberMissingPosition) === 1,
        'Fiber gameplay acquisition did not pin the generated chunk',
    );
    $generatedHandle->release();
    persistentJoinExpect(
        $store->chunkPinCount($fiberMissingPosition) === 0,
        'Fiber gameplay acquisition did not release the generated chunk pin',
    );

    $centerHandle = null;
    $server->scheduler()->spawn(
        static function () use ($server, $center, &$centerHandle): void {
            $centerHandle = $server->awaitResidentChunk($center);
        },
    );
    for ($attempt = 0; $attempt < 1000 && $centerHandle === null; ++$attempt) {
        $server->tick(1);
        usleep(1_000);
    }
    persistentJoinExpect($centerHandle !== null, 'center chunk could not be reacquired after eviction');
    $server->world()->setBlockStateId(new BlockPos(129, 5, 129), BlockStateId::fromLegacy(3));
    persistentJoinExpect($store->chunkDirty($center), 'server-stop flush probe did not start dirty');
    $centerHandle->release();
    $stopFlushDirty = true;
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
    if ($stopFlushDirty) {
        persistentJoinExpect(
            !$store->chunkDirty($center),
            'server stop did not durably flush the final dirty world state',
        );
    }

    persistentJoinExpect(
        $store->chunkPinCount($center) === 0,
        'persistent session shutdown did not release streamed chunk pins',
    );
    $store->destroy();
    persistentJoinRemoveTree($storageRoot);
}

fwrite(STDOUT, "persistent-join-smoke: passed\n");
