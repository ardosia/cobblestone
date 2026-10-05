<?php

declare(strict_types=1);

require dirname(__DIR__) . '/bootstrap.php';

use Cobblestone\Native\World as NativeWorld;
use Cobblestone\Server\WorldFactory;
use Cobblestone\Session\Internal\InitialChunkView;
use Cobblestone\World\BlockPos;
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
    infiniteExpect(
        [$spawn->x, $spawn->y, $spawn->z] === [820, 72, 4],
        'seed-0 target-resolved Infinite spawn mismatch',
    );

    $chunk = $memory->chunk(new ChunkPos(0, 0));
    infiniteExpect($chunk !== null, 'in-memory Infinite center did not generate');
    infiniteExpect($chunk->isGenerated(), 'Infinite center missing generated lifecycle');
    infiniteExpect($chunk->isPopulated(), 'Infinite center missing populated lifecycle');
    infiniteExpect($chunk->isLightPopulated(), 'Infinite center missing light-populated lifecycle');

    $snapshot = $chunk->snapshot();
    infiniteExpect($snapshot->biomeId(8, 8) === 7, 'seed-0 Infinite center biome mismatch');
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
$position = new ChunkPos(-28, -18);
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

$legacyRoot = sys_get_temp_dir()
    . '/cobblestone-native-infinite-spawn-migration-'
    . getmypid()
    . '-'
    . bin2hex(random_bytes(4));
try {
    $legacyStore = NativeWorld::create();
    try {
        $legacy = $legacyStore->attachStorage(
            $legacyRoot,
            'Legacy Infinite Spawn',
            -1_385_905_961,
            GeneratorType::Infinite->value,
            1,
            '',
            new BlockPos(396, 64, 32),
            saveWorkers: 1,
            loadWorkers: 1,
        );
        infiniteExpect($legacy->created, 'legacy Infinite migration fixture was not created');
        infiniteExpect($legacy->generatorSettingsVersion === 1, 'legacy fixture did not start at v1');
        infiniteExpect(
            [$legacy->spawn->x, $legacy->spawn->y, $legacy->spawn->z] === [396, 64, 32],
            'legacy fixture did not retain the historical provisional spawn',
        );
    } finally {
        $legacyStore->destroy();
    }

    $migrated = WorldFactory::persistentInfinite(
        $legacyRoot,
        'Ignored Migration Name',
        0,
        saveWorkers: 1,
        loadWorkers: 1,
    );
    try {
        infiniteExpect($migrated->seed() === -1_385_905_961, 'migration ignored stored seed');
        $spawn = $migrated->spawn();
        infiniteExpect(
            [$spawn->x, $spawn->y, $spawn->z] === [4, 63, 4],
            'legacy Infinite spawn did not migrate to target-resolved surface position',
        );
    } finally {
        $migrated->nativeStore()?->destroy();
    }

    $probeStore = NativeWorld::create();
    try {
        $metadata = $probeStore->attachStorage(
            $legacyRoot,
            'Ignored Probe Name',
            0,
            GeneratorType::Infinite->value,
            1,
            '',
            new BlockPos(0, 64, 0),
            saveWorkers: 1,
            loadWorkers: 1,
        );
        infiniteExpect(!$metadata->created, 'migrated Infinite metadata reopened as new');
        infiniteExpect($metadata->generatorSettingsVersion === 4, 'safe spawn migration did not persist v4');
        infiniteExpect(
            [$metadata->spawn->x, $metadata->spawn->y, $metadata->spawn->z] === [4, 63, 4],
            'safe spawn migration did not persist resolved coordinates',
        );
    } finally {
        $probeStore->destroy();
    }
} finally {
    infiniteRemoveTree($legacyRoot);
}

foreach ([2, 3] as $legacyVersion) {
    $recoveryRoot = sys_get_temp_dir()
        . "/cobblestone-native-infinite-spawn-v{$legacyVersion}-migration-"
        . getmypid()
        . '-'
        . bin2hex(random_bytes(4));
    try {
        $legacyStore = NativeWorld::create();
        try {
            $legacy = $legacyStore->attachStorage(
                $recoveryRoot,
                "Interrupted Spawn Migration v{$legacyVersion}",
                -1_385_905_961,
                GeneratorType::Infinite->value,
                $legacyVersion,
                '',
                new BlockPos(396, 74, 32),
                saveWorkers: 1,
                loadWorkers: 1,
            );
            infiniteExpect(
                $legacy->created,
                "v{$legacyVersion} Infinite migration fixture was not created",
            );
            infiniteExpect(
                $legacy->generatorSettingsVersion === $legacyVersion,
                "v{$legacyVersion} fixture did not retain its version",
            );
        } finally {
            $legacyStore->destroy();
        }

        $recovered = WorldFactory::persistentInfinite(
            $recoveryRoot,
            'Ignored Recovery Name',
            0,
            saveWorkers: 1,
            loadWorkers: 1,
        );
        try {
            $spawn = $recovered->spawn();
            infiniteExpect(
                [$spawn->x, $spawn->y, $spawn->z] === [4, 63, 4],
                "v{$legacyVersion} Infinite spawn recovery did not recompute target X/Z/Y",
            );
        } finally {
            $recovered->nativeStore()?->destroy();
        }

        $probeStore = NativeWorld::create();
        try {
            $metadata = $probeStore->attachStorage(
                $recoveryRoot,
                'Ignored Recovery Probe',
                0,
                GeneratorType::Infinite->value,
                1,
                '',
                new BlockPos(0, 64, 0),
                saveWorkers: 1,
                loadWorkers: 1,
            );
            infiniteExpect(
                $metadata->generatorSettingsVersion === 4,
                "v{$legacyVersion} recovery did not persist v4",
            );
            infiniteExpect(
                [$metadata->spawn->x, $metadata->spawn->y, $metadata->spawn->z] === [4, 63, 4],
                "v{$legacyVersion} recovery did not persist corrected spawn",
            );
        } finally {
            $probeStore->destroy();
        }
    } finally {
        infiniteRemoveTree($recoveryRoot);
    }
}

// A generated-only neighbor is stored as part of another center's 3x3 population view.
// When requested as a center, it must run its own structure post-process before delivery.
$villageRoot = sys_get_temp_dir()
    . '/cobblestone-native-infinite-village-neighbor-'
    . getmypid()
    . '-'
    . bin2hex(random_bytes(4));
try {
    $village = WorldFactory::persistentInfinite(
        $villageRoot,
        'Village Neighbor Smoke',
        -1_385_905_961,
        saveWorkers: 1,
        loadWorkers: 1,
    );
    $villageStore = $village->nativeStore()
        ?? throw new RuntimeException('village test requires native storage');
    infiniteAwaitChunk($village, new ChunkPos(0, 8));
    $well = new ChunkPos(1, 8); // Independent 0.15.10 Village source (1,8), Well bounds x18..23, z130..135.
    infiniteExpect($villageStore->lifecycleFlags($well) === Chunk::LIFECYCLE_GENERATED,
        'cross-chunk village Well was not left generated-only before requesting its center');
    $other = new ChunkPos(-1, 8);
    $otherChunk = $village->adoptNativeChunk($other);
    infiniteExpect(!$otherChunk->isPopulated(), 'neighbor fixture was already populated');
    $completed = infiniteAwaitChunk($village, $other);
    infiniteExpect($completed === $otherChunk && $completed->isPopulated() && $completed->isLightPopulated(),
        'already-adopted generated-only village neighbor was returned without population');

    $villageStore->flushStorage();
    $villageStore->destroy();

    $reopenedVillage = WorldFactory::persistentInfinite(
        $villageRoot,
        'Ignored Village Reopen',
        0,
        saveWorkers: 1,
        loadWorkers: 1,
    );
    try {
        $initialView = new InitialChunkView($reopenedVillage);
        $projection = NativeWorld::encodeStorageLoadBatch([$well]);
        $prepared = false;
        for ($attempt = 0; $attempt < 1000; ++$attempt) {
            if ($initialView->preparePersistent([$well], $projection)) {
                $prepared = true;
                break;
            }
            $reopenedVillage->nativeStore()?->storageTick(64);
            usleep(1_000);
        }
        infiniteExpect($prepared, 'persisted generated-only Well did not finish initial view preparation');
        $wellChunk = $reopenedVillage->chunk($well, false);
        infiniteExpect($wellChunk !== null && $wellChunk->isPopulated() && $wellChunk->isLightPopulated(),
            'persisted generated-only village Well was sent without population');
        $wellStates = $wellChunk->snapshot()->blockIds;
        infiniteExpect(str_contains($wellStates, chr(85)),
            'village Well fence was not placed in its center chunk');

        $resident = new ChunkPos(1, 9);
        $residentChunk = infiniteAwaitChunk($reopenedVillage, $resident);
        infiniteExpect($residentChunk->isPopulated() && $residentChunk->isLightPopulated(),
            'loaded generated-only neighbor was returned without population');

    } finally {
        $reopenedVillage->nativeStore()?->destroy();
    }
} finally {
    infiniteRemoveTree($villageRoot);
}

fwrite(STDOUT, "native-infinite-smoke: passed\n");
