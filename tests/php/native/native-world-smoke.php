<?php

declare(strict_types=1);

require dirname(__DIR__) . '/bootstrap.php';

use Cobblestone\Native\World as NativeWorld;
use Cobblestone\Server\WorldFactory;
use Cobblestone\World\BiomeColumn;
use Cobblestone\World\BiomeId;
use Cobblestone\World\BlockPos;
use Cobblestone\World\BlockStateId;
use Cobblestone\World\BlockType;
use Cobblestone\World\Chunk;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\ChunkUnloadStatus;
use Cobblestone\World\Light\LightEngine;
use Cobblestone\World\LightLayer;
use Cobblestone\World\LightUpdate;
use Cobblestone\World\WorldEdit;

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
nativeWorldExpect($store->handle() !== 0, 'native world handle was zero');

$foreignStore = NativeWorld::create();
try {
    try {
        $world->chunks()->put(new Chunk(new ChunkPos(99, 99), $foreignStore));
        throw new RuntimeException('native chunk source accepted a chunk from a different store');
    } catch (LogicException $error) {
        nativeWorldExpect(
            str_contains($error->getMessage(), 'source/store mismatch'),
            'native chunk source rejected mismatched storage for the wrong reason',
        );
    }
} finally {
    $foreignStore->destroy();
}

for ($chunkX = -1; $chunkX <= 1; ++$chunkX) {
    for ($chunkZ = -1; $chunkZ <= 1; ++$chunkZ) {
        nativeWorldExpect(
            $world->chunk(new ChunkPos($chunkX, $chunkZ)) !== null,
            "failed to generate native chunk {$chunkX}:{$chunkZ}",
        );
    }
}

$evictPosition = new ChunkPos(1, 1);
$evictChunk = $world->chunk($evictPosition, false);
nativeWorldExpect($evictChunk !== null, 'native eviction probe chunk was not resident');
nativeWorldExpect($evictChunk->isGenerated(), 'native generated lifecycle flag missing');
nativeWorldExpect($evictChunk->isPopulated(), 'native populated lifecycle flag missing');
nativeWorldExpect($evictChunk->isDirty(), 'new native generated chunk must start dirty');
$evictHandle = $world->pinChunk($evictPosition, false);
nativeWorldExpect($evictHandle !== null, 'native chunk lease missing');
nativeWorldExpect($store->chunkPinCount($evictPosition) === 1, 'native chunk lease did not pin');
nativeWorldExpect(
    $world->chunks()->unload($evictPosition) === ChunkUnloadStatus::Pinned,
    'native pin did not block safe unload',
);
$evictHandle->release();
nativeWorldExpect($store->chunkPinCount($evictPosition) === 0, 'native resident pin did not release');
nativeWorldExpect(
    $world->chunks()->unload($evictPosition) === ChunkUnloadStatus::Dirty,
    'native dirty chunk did not block safe unload',
);
$evictChunk->markCurrentStatePersisted();
nativeWorldExpect(!$evictChunk->isDirty(), 'native persisted watermark did not clean chunk');
nativeWorldExpect(
    $world->chunks()->unload($evictPosition) === ChunkUnloadStatus::Unloaded,
    'native clean unpinned chunk did not unload',
);
nativeWorldExpect($world->chunk($evictPosition, false) === null, 'native unloaded chunk stayed resident');

$chunk = $world->chunk(new ChunkPos(0, 0), false);
nativeWorldExpect($chunk !== null, 'center native chunk was not resident');
nativeWorldExpect($chunk->matchesNativeStore($store), 'chunk did not retain the world native store');
nativeWorldExpect($chunk->revision() === 0, 'generated native terrain revision must start at zero');
nativeWorldExpect(
    $chunk->lightRevision()->value === 0,
    'generated native light revision must start at zero',
);

try {
    $store->setBlockStateId(new ChunkPos(0, 0), 8, 20, 8, 36 << 4);
    throw new RuntimeException('native world store accepted unsupported block id 36');
} catch (Throwable $error) {
    nativeWorldExpect(
        str_contains($error->getMessage(), 'unsupported MCPE 0.15.10 block state id'),
        'native world store rejected unsupported block id for the wrong reason',
    );
}

$torchPosition = new BlockPos(8, 20, 8);
$torchState = BlockStateId::encode(BlockType::Torch);
nativeWorldExpect(
    $world->blockStateId($torchPosition) === BlockStateId::encode(BlockType::Air),
    'flat native chunk expected air above the floor',
);

$before = $chunk->snapshot();
$index = ($torchPosition->y << 8)
    | ($torchPosition->localZ() << 4)
    | $torchPosition->localX();
nativeWorldExpect(ord($before->blockIds[$index]) === 0, 'pre-write native snapshot was not air');

$previous = $world->setBlockStateId($torchPosition, $torchState);
nativeWorldExpect($previous === BlockStateId::encode(BlockType::Air), 'scalar native mutation previous state mismatch');
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
$world->edit(
    static function (WorldEdit $edit) use ($second): void {
        $edit->setBlockStateId($second, BlockStateId::encode(BlockType::Grass));
        $edit->setBlockExtraData($second, 0x1234);
    },
);
nativeWorldExpect($world->blockStateId($second) === BlockStateId::encode(BlockType::Grass), 'compound scalar state mismatch');
nativeWorldExpect($world->blockExtraData($second) === 0x1234, 'compound native extra-data mismatch');
nativeWorldExpect($chunk->revision() === 2, 'compound native mutation did not advance terrain revision once');
nativeWorldExpect(
    $chunk->lightRevision()->value === 1,
    'terrain-only compound mutation changed native light revision',
);

$world->edit(
    static function (WorldEdit $edit): void {
        for ($index = 0; $index < 800; ++$index) {
            $column = $index & 0xff;
            $position = new BlockPos(
                $column & 0x0f,
                21 + intdiv($index, 256),
                ($column >> 4) & 0x0f,
            );
            nativeWorldExpect(
                $edit->setBlockStateId($position, BlockStateId::encode(BlockType::Planks)) === BlockStateId::encode(BlockType::Air),
                'bulk native mutation previous state mismatch',
            );
        }
    },
);
nativeWorldExpect($chunk->revision() === 3, 'bulk native mutation did not advance terrain revision once');
foreach ([0, 255, 256, 767, 799] as $index) {
    $column = $index & 0xff;
    nativeWorldExpect(
        $world->blockStateId(new BlockPos(
            $column & 0x0f,
            21 + intdiv($index, 256),
            ($column >> 4) & 0x0f,
        )) === BlockStateId::encode(BlockType::Planks),
        'bulk native mutation state mismatch',
    );
}

$edit = $chunk->terrain()->edit();
nativeWorldExpect(
    $edit->setBlockStateId(10, 20, 8, BlockStateId::encode(BlockType::Cobblestone)) === BlockStateId::encode(BlockType::Air),
    'native terrain edit previous state mismatch',
);
$editChanged = $edit->commit();
nativeWorldExpect($editChanged, 'native terrain edit reported no change');
nativeWorldExpect($chunk->revision() === 4, 'native terrain edit did not advance revision once');
nativeWorldExpect(
    $chunk->blockStateId(10, 20, 8) === BlockStateId::encode(BlockType::Cobblestone),
    'native terrain edit state did not commit',
);

nativeWorldExpect($chunk->biomeColor(0, 0) === 0x92bc59, 'native flat biome color mismatch');
nativeWorldExpect(
    $world->setBiomeColorAt(0, 0, 0x123456) === 0x92bc59,
    'native biome color previous value mismatch',
);
nativeWorldExpect(
    $world->setBiomeAt(0, 0, new BiomeId(BiomeId::DESERT))->value === BiomeId::PLAINS,
    'native biome id previous value mismatch',
);
nativeWorldExpect($world->biomeColorAt(0, 0) === 0x123456, 'native biome id edit did not preserve color');
nativeWorldExpect(
    $world->setBiomeColumnAt(
        0,
        0,
        new BiomeColumn(new BiomeId(BiomeId::FOREST), 0xabcdef),
    )->word() === 0x02123456,
    'native full biome-column previous value mismatch',
);
$biomeSnapshot = $chunk->snapshot();
nativeWorldExpect($biomeSnapshot->biomeWord(0, 0) === 0x04abcdef, 'native snapshot biome word mismatch');
nativeWorldExpect($biomeSnapshot->biomeColor(0, 0) === 0xabcdef, 'native snapshot biome color mismatch');

fwrite(STDOUT, "native-world-smoke: passed\n");
