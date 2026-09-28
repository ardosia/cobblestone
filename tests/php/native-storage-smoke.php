<?php

declare(strict_types=1);

require __DIR__ . '/bootstrap.php';

use Cobblestone\World\BiomeId;
use Cobblestone\World\BlockPos;
use Cobblestone\World\Chunk;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\NativeChunkLoadStatus;
use Cobblestone\World\NativeWorldStore;

function nativeStorageExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

function nativeStorageRemoveTree(string $path): void
{
    if (!is_dir($path)) {
        return;
    }

    $entries = scandir($path);
    if ($entries === false) {
        throw new RuntimeException("failed to scan {$path}");
    }
    foreach ($entries as $entry) {
        if ($entry === '.' || $entry === '..') {
            continue;
        }
        $child = $path . DIRECTORY_SEPARATOR . $entry;
        if (is_dir($child)) {
            nativeStorageRemoveTree($child);
        } elseif (!unlink($child)) {
            throw new RuntimeException("failed to remove {$child}");
        }
    }
    if (!rmdir($path)) {
        throw new RuntimeException("failed to remove {$path}");
    }
}

$root = sys_get_temp_dir()
    . '/cobblestone-native-storage-'
    . getmypid()
    . '-'
    . bin2hex(random_bytes(4));

$store = NativeWorldStore::create();

try {
    $metadata = $store->attachStorage(
        $root,
        'Storage Smoke',
        424242,
        2,
        1,
        '2;7,2x3,2;1;',
        new BlockPos(128, 4, 128),
        saveWorkers: 2,
        createUuid: str_repeat("Z", 16),
    );
    nativeStorageExpect($metadata->created, 'first native storage attach did not create world.cwm');
    nativeStorageExpect($metadata->name === 'Storage Smoke', 'created metadata name mismatch');
    nativeStorageExpect($metadata->seed === 424242, 'created metadata seed mismatch');
    nativeStorageExpect($metadata->generatorId === 2, 'created generator id mismatch');
    nativeStorageExpect($metadata->generatorSettings === '2;7,2x3,2;1;', 'created preset mismatch');
    nativeStorageExpect($metadata->uuid === str_repeat("Z", 16), 'created UUID mismatch');
    nativeStorageExpect(is_file($root . '/world.cwm'), 'world.cwm was not created');

    $position = new ChunkPos(0, 0);
    nativeStorageExpect($store->ensureChunk($position, new BiomeId(1)), 'storage probe chunk already existed');
    $store->fillLayers($position, 0, 1, 7 << 4);
    $store->fillLayers($position, 1, 2, 3 << 4);
    $store->fillLayers($position, 3, 1, 2 << 4);
    $store->setLifecycleFlags(
        $position,
        Chunk::LIFECYCLE_GENERATED
            | Chunk::LIFECYCLE_POPULATED
            | Chunk::LIFECYCLE_LIGHT_POPULATED,
    );
    nativeStorageExpect($store->chunkDirty($position), 'new persistent chunk did not start dirty');

    $scheduled = false;
    for ($attempt = 0; $attempt < 500; ++$attempt) {
        $tick = $store->storageTick(64);
        $scheduled = $scheduled || $tick['scheduled'] > 0;
        if (!$store->chunkDirty($position) && $tick['in_flight'] === 0) {
            break;
        }
        usleep(1_000);
    }

    nativeStorageExpect($scheduled, 'native storage tick never scheduled the dirty chunk');
    nativeStorageExpect(!$store->chunkDirty($position), 'durable save receipt did not clear dirty watermark');
    $regionPath = $root . '/regions/r.0.0.cwr';
    nativeStorageExpect(is_file($regionPath), 'native storage did not create region file');
    clearstatcache(true, $regionPath);
    $beforeShutdownFlush = filesize($regionPath);
    nativeStorageExpect($beforeShutdownFlush !== false, 'failed to stat native region before shutdown flush');

    $shutdownPosition = new ChunkPos(1, 0);
    nativeStorageExpect(
        $store->ensureChunk($shutdownPosition, new BiomeId(1)),
        'shutdown-flush probe chunk already existed',
    );
    $store->fillLayers($shutdownPosition, 0, 1, 7 << 4);
    $store->setLifecycleFlags($shutdownPosition, Chunk::LIFECYCLE_GENERATED);
    nativeStorageExpect(
        $store->chunkDirty($shutdownPosition),
        'shutdown-flush probe chunk did not start dirty',
    );

    $store->destroy();
    clearstatcache(true, $regionPath);
    $afterShutdownFlush = filesize($regionPath);
    nativeStorageExpect($afterShutdownFlush !== false, 'failed to stat native region after shutdown flush');
    nativeStorageExpect(
        $afterShutdownFlush > $beforeShutdownFlush,
        'world destroy did not persist unscheduled dirty chunk',
    );

    $reopened = NativeWorldStore::create();
    try {
        $metadata = $reopened->attachStorage(
            $root,
            'Ignored Creation Name',
            -999,
            0,
            99,
            'ignored',
            new BlockPos(0, 0, 0),
            saveWorkers: 1,
        );
        nativeStorageExpect(!$metadata->created, 'existing native storage was recreated');
        nativeStorageExpect($metadata->name === 'Storage Smoke', 'reopen did not trust stored world name');
        nativeStorageExpect($metadata->seed === 424242, 'reopen did not trust stored world seed');
        nativeStorageExpect($metadata->generatorId === 2, 'reopen did not trust stored generator id');
        nativeStorageExpect($metadata->generatorSettings === '2;7,2x3,2;1;', 'reopen preset mismatch');
        nativeStorageExpect($metadata->uuid === str_repeat("Z", 16), 'reopen UUID mismatch');

        $bulkMissing = new ChunkPos(60, -60);
        $bulkProjection = NativeWorldStore::encodeStorageLoadBatch([$position, $bulkMissing]);
        $bulk = $reopened->prepareStorageLoadBatch($bulkProjection);
        nativeStorageExpect(
            $bulk === chr(NativeChunkLoadStatus::Queued->value) . chr(NativeChunkLoadStatus::Queued->value),
            'bulk native load preparation did not queue both unresolved chunks',
        );
        for ($attempt = 0; $attempt < 500; ++$attempt) {
            $bulk = $reopened->prepareStorageLoadBatch($bulkProjection);
            $persistedStatus = NativeChunkLoadStatus::from(ord($bulk[0]));
            $missingStatus = NativeChunkLoadStatus::from(ord($bulk[1]));
            if (
                $persistedStatus === NativeChunkLoadStatus::Resident
                && $missingStatus === NativeChunkLoadStatus::Missing
            ) {
                break;
            }
            nativeStorageExpect(
                $persistedStatus !== NativeChunkLoadStatus::Missing,
                'persisted chunk unexpectedly became a durable miss',
            );
            nativeStorageExpect(
                $missingStatus !== NativeChunkLoadStatus::Resident,
                'durably missing bulk chunk unexpectedly became resident',
            );
            usleep(1_000);
        }
        nativeStorageExpect(
            $bulk === chr(NativeChunkLoadStatus::Resident->value) . chr(NativeChunkLoadStatus::Missing->value),
            'bulk native load preparation did not converge to resident/missing states',
        );
        nativeStorageExpect(
            $reopened->requestStorageLoad($position) === NativeChunkLoadStatus::Resident,
            'persisted chunk was not imported into WorldStore',
        );
        nativeStorageExpect(
            $reopened->blockStateId($position, 0, 0, 0) === (7 << 4),
            'loaded persistent chunk block state mismatch',
        );
        nativeStorageExpect(
            !$reopened->chunkDirty($position),
            'loaded persistent chunk did not enter residency clean',
        );

        $missing = new ChunkPos(50, -50);
        nativeStorageExpect(
            $reopened->requestStorageLoad($missing) === NativeChunkLoadStatus::Queued,
            'missing chunk load was not queued',
        );
        for ($attempt = 0; $attempt < 500; ++$attempt) {
            $reopened->storageTick(64);
            $status = $reopened->requestStorageLoad($missing);
            if ($status === NativeChunkLoadStatus::Missing) {
                break;
            }
            nativeStorageExpect(
                $status !== NativeChunkLoadStatus::Resident,
                'durably missing chunk became resident before generation',
            );
            usleep(1_000);
        }
        nativeStorageExpect(
            $reopened->requestStorageLoad($missing) === NativeChunkLoadStatus::Missing,
            'durable missing chunk did not expose Missing state',
        );
        nativeStorageExpect(
            $reopened->ensureChunk($missing, new BiomeId(1)),
            'generation after durable miss did not create native chunk',
        );
        nativeStorageExpect(
            $reopened->requestStorageLoad($missing) === NativeChunkLoadStatus::Resident,
            'generated chunk did not clear durable-miss state',
        );
        nativeStorageExpect(
            $reopened->chunkDirty($missing),
            'generated chunk after durable miss was not dirty',
        );
    } finally {
        $reopened->destroy();
    }
} finally {
    try {
        $store->destroy();
    } catch (Throwable) {
    }
    nativeStorageRemoveTree($root);
}

fwrite(STDOUT, "native-storage-smoke: passed\n");
