<?php

declare(strict_types=1);

require __DIR__ . '/bootstrap.php';

use Cobblestone\Server\WorldFactory;
use Cobblestone\World\BlockPos;
use Cobblestone\World\BlockStateId;
use Cobblestone\World\Chunk;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\Light\LightEngine;
use Cobblestone\World\LightLayer;
use Cobblestone\World\LightUpdate;
use Cobblestone\World\Mutation\WorldMutation;

function nativeWorldExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

nativeWorldExpect(
    extension_loaded('cobblestone_core_php'),
    'native-world smoke requires cobblestone_core_php',
);

$world = WorldFactory::flat('Native World Smoke', 4242);
$store = $world->nativeStore();
nativeWorldExpect($store !== null, 'WorldFactory did not select the native world store');
nativeWorldExpect($store->handle() !== 0, 'native world handle was zero');

try {
    $world->chunks()->put(new Chunk(new ChunkPos(99, 99)));
    throw new RuntimeException('native chunk source accepted a chunk from a different store');
} catch (LogicException $error) {
    nativeWorldExpect(
        str_contains($error->getMessage(), 'source/store mismatch'),
        'native chunk source rejected mismatched storage for the wrong reason',
    );
}

for ($chunkX = -1; $chunkX <= 1; ++$chunkX) {
    for ($chunkZ = -1; $chunkZ <= 1; ++$chunkZ) {
        nativeWorldExpect(
            $world->chunk(new ChunkPos($chunkX, $chunkZ)) !== null,
            "failed to generate native chunk {$chunkX}:{$chunkZ}",
        );
    }
}

$chunk = $world->chunk(new ChunkPos(0, 0), false);
nativeWorldExpect($chunk !== null, 'center native chunk was not resident');
nativeWorldExpect($chunk->nativeStore() === $store, 'chunk did not retain the world native store');
nativeWorldExpect($chunk->revision() === 0, 'generated native terrain revision must start at zero');
nativeWorldExpect(
    $chunk->lightRevision()->value === 0,
    'generated native light revision must start at zero',
);

$torchPosition = new BlockPos(8, 20, 8);
$torchState = BlockStateId::fromLegacy(50);
nativeWorldExpect(
    $world->blockStateId($torchPosition) === BlockStateId::fromLegacy(0),
    'flat native chunk expected air above the floor',
);

$before = $chunk->snapshot();
$index = ($torchPosition->y << 8)
    | ($torchPosition->localZ() << 4)
    | $torchPosition->localX();
nativeWorldExpect(ord($before->blockIds[$index]) === 0, 'pre-write native snapshot was not air');

$previous = $world->setBlockStateId($torchPosition, $torchState);
nativeWorldExpect($previous === BlockStateId::fromLegacy(0), 'scalar native mutation previous state mismatch');
nativeWorldExpect($world->blockStateId($torchPosition) === $torchState, 'scalar native mutation did not commit');
nativeWorldExpect($chunk->revision() === 1, 'native scalar mutation did not advance terrain revision once');
nativeWorldExpect(
    ord($before->blockIds[$index]) === 0,
    'immutable native snapshot changed after later terrain mutation',
);
$afterWrite = $chunk->snapshot();
nativeWorldExpect(ord($afterWrite->blockIds[$index]) === 50, 'native snapshot missed committed torch');
nativeWorldExpect(
    $afterWrite->blockStateId(8, 20, 8) === $torchState,
    'scalar native snapshot state mismatch',
);
nativeWorldExpect(
    $afterWrite->terrain()->blockStateId(8, 20, 8) === $torchState,
    'scalar terrain snapshot state mismatch',
);
nativeWorldExpect(
    $afterWrite->blockLightLevel(8, 20, 8) === 0,
    'scalar native snapshot block-light mismatch',
);

$engine = LightEngine::fixedTarget();
$lightResult = $engine->apply(
    $world,
    LightUpdate::point(LightLayer::Block, $torchPosition),
);
nativeWorldExpect($lightResult->changed(), 'native-backed light propagation reported no changes');
nativeWorldExpect($world->blockLight($torchPosition) === 14, 'native-backed torch light mismatch');
nativeWorldExpect(
    $world->blockLight(new BlockPos(9, 20, 8)) === 13,
    'native-backed neighbor light attenuation mismatch',
);
nativeWorldExpect(
    $chunk->lightRevision()->value === 1,
    'native-backed light propagation did not batch one center-chunk light revision',
);

$second = new BlockPos(9, 20, 8);
$compound = $world->mutate(
    static function (WorldMutation $mutation) use ($second): void {
        $mutation->setBlockStateId($second, BlockStateId::fromLegacy(2));
        $mutation->setBlockExtraData($second, 0x1234);
    },
);
nativeWorldExpect($compound->changed(), 'native compound scalar mutation reported no change');
nativeWorldExpect($world->blockStateId($second) === BlockStateId::fromLegacy(2), 'compound scalar state mismatch');
nativeWorldExpect($world->blockExtraData($second) === 0x1234, 'compound native extra-data mismatch');
nativeWorldExpect($chunk->revision() === 2, 'compound native mutation did not advance terrain revision once');
nativeWorldExpect(
    $chunk->lightRevision()->value === 1,
    'terrain-only compound mutation changed native light revision',
);

$bulk = $world->mutate(
    static function (WorldMutation $mutation): void {
        for ($index = 0; $index < 800; ++$index) {
            $column = $index & 0xff;
            $position = new BlockPos(
                $column & 0x0f,
                21 + intdiv($index, 256),
                ($column >> 4) & 0x0f,
            );
            nativeWorldExpect(
                $mutation->setBlockStateId($position, BlockStateId::fromLegacy(5)) === BlockStateId::fromLegacy(0),
                'bulk native mutation previous state mismatch',
            );
        }
    },
);
nativeWorldExpect($bulk->changed(), 'bulk native mutation reported no change');
nativeWorldExpect($chunk->revision() === 3, 'bulk native mutation did not advance terrain revision once');
foreach ([0, 255, 256, 767, 799] as $index) {
    $column = $index & 0xff;
    nativeWorldExpect(
        $world->blockStateId(new BlockPos(
            $column & 0x0f,
            21 + intdiv($index, 256),
            ($column >> 4) & 0x0f,
        )) === BlockStateId::fromLegacy(5),
        'bulk native mutation state mismatch',
    );
}

$edit = $chunk->terrain()->edit();
nativeWorldExpect(
    $edit->setBlockStateId(10, 20, 8, BlockStateId::fromLegacy(4)) === BlockStateId::fromLegacy(0),
    'native terrain edit previous state mismatch',
);
$editResult = $edit->commit();
nativeWorldExpect($editResult->changed, 'native terrain edit reported no change');
nativeWorldExpect($chunk->revision() === 4, 'native terrain edit did not advance revision once');
nativeWorldExpect(
    $chunk->blockStateId(10, 20, 8) === BlockStateId::fromLegacy(4),
    'native terrain edit state did not commit',
);

fwrite(STDOUT, "native-world-smoke: passed\n");
