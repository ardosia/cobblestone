<?php

declare(strict_types=1);

require __DIR__ . '/bootstrap.php';

use Cobblestone\Server\WorldFactory;
use Cobblestone\World\BlockPos;
use Cobblestone\World\BlockState;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\Generator\FlatGenerator;
use Cobblestone\World\Generator\FlatPreset;
use Cobblestone\World\Generator\GeneratorType;
use Cobblestone\World\WorldBounds;

function worldExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

function worldNibble(string $bytes, int $index): int
{
    $value = ord($bytes[$index >> 1]);
    return ($index & 1) === 0 ? $value & 0x0f : ($value >> 4) & 0x0f;
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

$world = WorldFactory::create('Cobblestone', 12345, $generator);
$chunk = $world->chunk(new ChunkPos(-1, 2));
worldExpect($chunk !== null, 'generated chunk missing');
worldExpect($chunk->isGenerated(), 'generated flag missing');
worldExpect($chunk->isPopulated(), 'populated flag missing');
worldExpect($chunk->isLightPopulated(), 'flat generator should populate fixed-target sky light');
worldExpect($chunk->block(0, 0, 0)->id === 7, 'flat bedrock layer mismatch');
worldExpect($chunk->block(0, 1, 0)->id === 3, 'flat dirt layer 1 mismatch');
worldExpect($chunk->block(0, 2, 0)->id === 3, 'flat dirt layer 2 mismatch');
worldExpect($chunk->block(0, 3, 0)->id === 2, 'flat grass layer mismatch');
worldExpect($chunk->block(0, 4, 0)->isAir(), 'flat air layer mismatch');
worldExpect($chunk->highestBlockAt(0, 0) === 3, 'flat height map source mismatch');
worldExpect($chunk->heightMap(0, 0) === 3, 'flat height map cache mismatch');
worldExpect($chunk->biome(0, 0)->value === 1, 'flat biome column mismatch');
worldExpect($chunk->skyLight(0, 3, 0) === 0, 'flat surface sky-light mismatch');
worldExpect($chunk->skyLight(0, 4, 0) === 15, 'flat first-air sky-light mismatch');

$snapshot = $chunk->snapshot();
worldExpect($snapshot->position->x === -1 && $snapshot->position->z === 2, 'snapshot position mismatch');
worldExpect($snapshot->revision === 0, 'fresh generated snapshot revision mismatch');
worldExpect(strlen($snapshot->blockIds) === 32768, 'snapshot block-id plane length mismatch');
worldExpect(strlen($snapshot->blockData) === 16384, 'snapshot block-data plane length mismatch');
worldExpect(strlen($snapshot->skyLight) === 16384, 'snapshot sky-light plane length mismatch');
worldExpect(strlen($snapshot->blockLight) === 16384, 'snapshot block-light plane length mismatch');
worldExpect(ord($snapshot->blockIds[0]) === 7, 'snapshot bedrock byte mismatch');
worldExpect(ord($snapshot->blockIds[256]) === 3, 'snapshot first dirt byte mismatch');
worldExpect(ord($snapshot->blockIds[512]) === 3, 'snapshot second dirt byte mismatch');
worldExpect(ord($snapshot->blockIds[768]) === 2, 'snapshot grass byte mismatch');
worldExpect(ord($snapshot->blockIds[1024]) === 0, 'snapshot first-air byte mismatch');
worldExpect(worldNibble($snapshot->skyLight, 768) === 0, 'snapshot surface sky-light mismatch');
worldExpect(worldNibble($snapshot->skyLight, 1024) === 15, 'snapshot first-air sky-light mismatch');
worldExpect(ord($snapshot->biomes[0]) === 1, 'snapshot biome-id byte mismatch');
worldExpect(ord($snapshot->heightMap[0]) === 3, 'snapshot height-map byte mismatch');
worldExpect($snapshot->extraData === [], 'fresh snapshot should not contain extra data');

$global = new BlockPos(-1, 10, 47);
worldExpect($global->chunk()->x === -1 && $global->chunk()->z === 2, 'global-to-chunk mapping mismatch');
worldExpect($global->localX() === 15 && $global->localZ() === 15, 'global local coordinate mismatch');
$previous = $world->setBlock($global, new BlockState(5, 2));
worldExpect($previous->isAir(), 'world block replacement previous state mismatch');
worldExpect($world->block($global)->fullId() === ((5 << 4) | 2), 'world block lookup mismatch');
worldExpect($chunk->heightMap(15, 15) === 10, 'height cache did not advance after world write');

$mid = new BlockPos(-1, 70, 47);
$high = new BlockPos(-1, 80, 47);
$world->setBlock($mid, new BlockState(1));
$world->setBlock($high, new BlockState(1));
worldExpect($chunk->highestBlockAt(15, 15) === 80, 'section height cache did not find highest write');
worldExpect($chunk->heightMap(15, 15) === 80, 'chunk height cache did not track highest write');
$world->setBlock($high, BlockState::air());
worldExpect($chunk->highestBlockAt(15, 15) === 70, 'height cache did not fall to next occupied section');
worldExpect($chunk->heightMap(15, 15) === 70, 'chunk height cache did not fall after removal');
$world->setBlock($mid, BlockState::air());
worldExpect($chunk->highestBlockAt(15, 15) === 10, 'height cache did not fall to lower occupied section');

worldExpect($chunk->setBlockExtraData(15, 10, 15, 0xbeef) === 0, 'extra data previous value mismatch');
worldExpect($chunk->blockExtraData(15, 10, 15) === 0xbeef, 'extra data round-trip mismatch');
worldExpect(($chunk->snapshot()->extraData[0xff0a] ?? null) === 0xbeef, 'snapshot extra data mismatch');
worldExpect($chunk->setSkyLight(15, 10, 15, 14) === 15, 'sky light previous value mismatch');
worldExpect($world->skyLight($global) === 14, 'sky light world lookup mismatch');

fwrite(STDOUT, "world-smoke: passed\n");
