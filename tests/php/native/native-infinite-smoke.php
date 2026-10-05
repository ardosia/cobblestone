<?php

declare(strict_types=1);

require dirname(__DIR__) . '/bootstrap.php';

use Cobblestone\Server\WorldFactory;
use Cobblestone\World\Chunk;
use Cobblestone\World\ChunkLoadPending;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\Generator\GeneratorType;

function infiniteExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

function infiniteRemoveTree(string $path): void
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
            infiniteRemoveTree($child);
        } elseif (!unlink($child)) {
            throw new RuntimeException("failed to remove {$child}");
        }
    }
    if (!rmdir($path)) {
        throw new RuntimeException("failed to remove {$path}");
    }
}

/** @return Chunk */
function infiniteAwaitChunk(
    \Cobblestone\World\World $world,
    ChunkPos $position,
    int $attempts = 1000,
): Chunk {
    $store = $world->nativeStore()
        ?? throw new RuntimeException('Infinite world did not expose native store');

    for ($attempt = 0; $attempt < $attempts; ++$attempt) {
        try {
            return $world->chunk($position)
                ?? throw new RuntimeException('Infinite chunk unexpectedly resolved to null');
        } catch (ChunkLoadPending) {
            $store->storageTick(64);
            usleep(1_000);
        }
    }

    throw new RuntimeException(
        "Infinite chunk {$position->x}:{$position->z} did not resolve from storage",
    );
}

function infiniteSnapshotHash(\Cobblestone\World\ChunkSnapshot $snapshot): string
{
    $extra = $snapshot->extraData;
    ksort($extra);

    return hash(
        'sha256',
        $snapshot->blockIds
        . $snapshot->blockData
        . $snapshot->skyLight
        . $snapshot->blockLight
        . $snapshot->biomeWords
        . $snapshot->heightMap
        . serialize($extra),
    );
}

infiniteExpect(
    extension_loaded('cobblestone_core_php'),
    'native Infinite smoke requires cobblestone_core_php',
);

$memory = WorldFactory::infinite('Infinite Memory Smoke', 0);
try {
    infiniteExpect($memory->generatorType() === GeneratorType::Infinite, 'Infinite generator type mismatch');
    $spawn = $memory->spawn();
    infiniteExpect($spawn->y === 64, 'temporary Infinite spawn Y projection mismatch');

    $chunk = $memory->chunk(new ChunkPos(0, 0));
    infiniteExpect($chunk !== null, 'in-memory Infinite center did not generate');
    infiniteExpect($chunk->isGenerated(), 'Infinite center missing generated lifecycle');
    infiniteExpect($chunk->isPopulated(), 'Infinite center missing populated lifecycle');
    infiniteExpect($chunk->isLightPopulated(), 'Infinite center missing light-populated lifecycle');

    $snapshot = $chunk->snapshot();
    infiniteExpect($snapshot->biomeId(8, 8) === 4, 'seed-0 Infinite center biome mismatch');
    infiniteExpect(
        $snapshot->heightMap !== str_repeat("\0", 256),
        'Infinite center height map remained empty',
    );
    infiniteExpect(
        $snapshot->skyLight !== str_repeat("\0", strlen($snapshot->skyLight)),
        'Infinite center skylight remained empty',
    );
} finally {
    $memory->nativeStore()?->destroy();
}

$root = sys_get_temp_dir()
    . '/cobblestone-native-infinite-'
    . getmypid()
    . '-'
    . bin2hex(random_bytes(4));
$position = new ChunkPos(-26, -21);
$beforeHash = null;

try {
    $world = WorldFactory::persistentInfinite(
        $root,
        'Infinite Persistent Smoke',
        42,
        saveWorkers: 2,
        loadWorkers: 2,
    );
    $store = $world->nativeStore()
        ?? throw new RuntimeException('persistent Infinite world did not expose native store');

    $chunk = infiniteAwaitChunk($world, $position);
    infiniteExpect($chunk->isGenerated(), 'persistent Infinite center missing generated lifecycle');
    infiniteExpect($chunk->isPopulated(), 'persistent Infinite center missing populated lifecycle');
    infiniteExpect($chunk->isLightPopulated(), 'persistent Infinite center missing light-populated lifecycle');
    infiniteExpect($chunk->snapshot()->biomeId(8, 8) === 12, 'cold fixture biome mismatch');

    $beforeHash = infiniteSnapshotHash($chunk->snapshot());
    $store->flushStorage();
    infiniteExpect(is_file($root . '/generator.cgs'), 'Infinite structure-state sidecar was not persisted');
    $store->destroy();

    $reopened = WorldFactory::persistentInfinite(
        $root,
        'Ignored Reopen Name',
        -999,
        saveWorkers: 1,
        loadWorkers: 1,
    );
    try {
        infiniteExpect($reopened->name() === 'Infinite Persistent Smoke', 'reopen ignored stored world name');
        infiniteExpect($reopened->seed() === 42, 'reopen ignored stored Infinite seed');
        infiniteExpect($reopened->generatorType() === GeneratorType::Infinite, 'reopen generator type mismatch');

        $reloaded = infiniteAwaitChunk($reopened, $position);
        infiniteExpect(
            infiniteSnapshotHash($reloaded->snapshot()) === $beforeHash,
            'persisted Infinite center changed after reopen',
        );
        infiniteExpect(
            $reloaded->isGenerated() && $reloaded->isPopulated() && $reloaded->isLightPopulated(),
            'reloaded Infinite lifecycle flags changed',
        );
    } finally {
        $reopened->nativeStore()?->destroy();
    }
} finally {
    infiniteRemoveTree($root);
}

fwrite(STDOUT, "native-infinite-smoke: passed\n");
