<?php

declare(strict_types=1);

require __DIR__ . '/bootstrap.php';

use Cobblestone\World\BlockPos;
use Cobblestone\World\BlockState;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\Generator\FlatGenerator;
use Cobblestone\World\Generator\FlatPreset;
use Cobblestone\World\Generator\GeneratorType;
use Cobblestone\World\World;
use Cobblestone\World\WorldBounds;

function worldExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

worldExpect(WorldBounds::CHUNK_EDGE === 16, 'chunk edge mismatch');
worldExpect(WorldBounds::SECTION_COUNT === 8, 'section count mismatch');
worldExpect(WorldBounds::WORLD_HEIGHT === 128, 'world height mismatch');

$negative = ChunkPos::fromBlock(-1, -17);
worldExpect($negative->x === -1 && $negative->z === -2, 'negative chunk floor division mismatch');
worldExpect(ChunkPos::localCoordinate(-1) === 15, 'negative local coordinate mismatch');

$preset = FlatPreset::default();
worldExpect($preset->toString() === FlatPreset::DEFAULT, 'default flat preset did not round-trip');
worldExpect($preset->floorLevel() === 4, 'default flat floor level mismatch');
worldExpect($preset->biome()->value === 1, 'default flat biome mismatch');

$generator = FlatGenerator::defaults();
worldExpect($generator->name() === 'flat', 'flat generator name mismatch');
worldExpect($generator->type() === GeneratorType::Flat, 'flat generator type mismatch');
worldExpect($generator->type()->value === 2, 'protocol-84 flat generator id mismatch');

$spawn = $generator->spawn();
worldExpect([$spawn->x, $spawn->y, $spawn->z] === [128, 4, 128], 'default flat spawn mismatch');

$world = new World('Cobblestone', 12345, $generator);
$chunk = $world->chunk(new ChunkPos(-1, 2));
worldExpect($chunk !== null, 'generated chunk missing');
worldExpect($chunk->isGenerated(), 'generated flag missing');
worldExpect($chunk->isPopulated(), 'populated flag missing');
worldExpect(!$chunk->isLightPopulated(), 'flat generator should not claim light population');
worldExpect($chunk->block(0, 0, 0)->id === 7, 'flat bedrock layer mismatch');
worldExpect($chunk->block(0, 1, 0)->id === 3, 'flat dirt layer 1 mismatch');
worldExpect($chunk->block(0, 2, 0)->id === 3, 'flat dirt layer 2 mismatch');
worldExpect($chunk->block(0, 3, 0)->id === 2, 'flat grass layer mismatch');
worldExpect($chunk->block(0, 4, 0)->isAir(), 'flat air layer mismatch');
worldExpect($chunk->highestBlockAt(0, 0) === 3, 'flat height map source mismatch');
worldExpect($chunk->heightMap(0, 0) === 3, 'flat height map cache mismatch');
worldExpect($chunk->biome(0, 0)->value === 1, 'flat biome column mismatch');

$global = new BlockPos(-1, 10, 47);
worldExpect($global->chunk()->x === -1 && $global->chunk()->z === 2, 'global-to-chunk mapping mismatch');
worldExpect($global->localX() === 15 && $global->localZ() === 15, 'global local coordinate mismatch');
$previous = $world->setBlock($global, new BlockState(5, 2));
worldExpect($previous->isAir(), 'world block replacement previous state mismatch');
worldExpect($world->block($global)->fullId() === ((5 << 4) | 2), 'world block lookup mismatch');

worldExpect($chunk->setBlockExtraData(15, 10, 15, 0xbeef) === 0, 'extra data previous value mismatch');
worldExpect($chunk->blockExtraData(15, 10, 15) === 0xbeef, 'extra data round-trip mismatch');
worldExpect($chunk->setSkyLight(15, 10, 15, 15) === 0, 'sky light previous value mismatch');
worldExpect($world->skyLight($global) === 15, 'sky light world lookup mismatch');

fwrite(STDOUT, "world-smoke: passed\n");
