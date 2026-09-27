<?php

declare(strict_types=1);

require __DIR__ . '/bootstrap.php';

use Cobblestone\Server\WorldFactory;
use Cobblestone\World\BlockPos;
use Cobblestone\World\BlockState;
use Cobblestone\World\BlockStateId;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\Light\BlockLightCatalog;
use Cobblestone\World\Light\LightEngine;
use Cobblestone\World\LightLayer;
use Cobblestone\World\LightUpdate;
use Cobblestone\World\SectionY;

function lightExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

lightExpect(SectionY::fromBlockY(-1) === null, 'negative block y produced a section');
lightExpect(SectionY::fromBlockY(0)?->value === 0, 'block y=0 section mismatch');
lightExpect(SectionY::fromBlockY(16)?->value === 1, 'block y=16 section mismatch');
lightExpect(SectionY::fromBlockY(127)?->value === 7, 'block y=127 section mismatch');
lightExpect(SectionY::fromBlockY(128) === null, 'block y=128 produced a section');

$catalog = new BlockLightCatalog();
$air = $catalog->propertiesForStateId(BlockStateId::fromLegacy(0));
$stone = $catalog->propertiesForStateId(BlockStateId::fromLegacy(1));
$torch = $catalog->propertiesForStateId(BlockStateId::fromLegacy(50));
lightExpect($air?->lightBlock === 0 && $air->lightEmission === 0, 'air light metadata mismatch');
lightExpect($stone?->lightBlock === 15, 'stone light-block metadata mismatch');
lightExpect(
    $torch?->lightBlock === 0 && $torch->lightEmission === 14,
    'torch binary-derived light metadata mismatch',
);

$world = WorldFactory::flat('Light Smoke', 123);
for ($chunkX = -1; $chunkX <= 1; ++$chunkX) {
    for ($chunkZ = -1; $chunkZ <= 1; ++$chunkZ) {
        $world->chunk(new ChunkPos($chunkX, $chunkZ), true);
    }
}

$chunk = $world->chunk(new ChunkPos(0, 0), false);
lightExpect($chunk !== null, 'center chunk was not resident');
lightExpect($chunk->terrainRevision()->value === 0, 'generated terrain revision must start at zero');
lightExpect($chunk->lightRevision()->value === 0, 'generated light revision must start at zero');

$position = new BlockPos(8, 20, 8);
$before = $chunk->lightSnapshot();
lightExpect($before->revision->value === 0, 'initial light snapshot revision mismatch');
lightExpect($before->block(8, 20, 8)?->value === 0, 'initial block light must be dark');
lightExpect($before->sky(8, 20, 8)?->value === 15, 'default Flat exposed sky light mismatch');

$world->setBlock($position, new BlockState(50));
$terrainRevision = $chunk->terrainRevision()->value;
lightExpect($terrainRevision === 1, 'torch placement did not advance terrain revision');
lightExpect($chunk->lightRevision()->value === 0, 'terrain write advanced light revision');

$engine = LightEngine::fixedTarget();
$result = $engine->apply($world, LightUpdate::point(LightLayer::Block, $position));
lightExpect($result->changed(), 'torch light propagation reported no changed chunks');
lightExpect($result->processedUpdates > 1, 'torch light propagation did not enqueue neighbor work');
lightExpect($world->blockLight($position) === 14, 'torch source light mismatch');
lightExpect(
    $world->blockLight(new BlockPos(9, 20, 8)) === 13,
    'torch neighbor attenuation mismatch',
);
lightExpect(
    $chunk->terrainRevision()->value === $terrainRevision,
    'light propagation advanced terrain revision',
);
lightExpect($chunk->lightRevision()->value === 1, 'light propagation did not advance light revision once');
lightExpect($before->block(8, 20, 8)?->value === 0, 'immutable light snapshot changed after propagation');

$after = $chunk->lightSnapshot();
lightExpect($after->revision->value === 1, 'post-propagation light snapshot revision mismatch');
lightExpect($after->block(8, 20, 8)?->value === 14, 'post-propagation snapshot source mismatch');

fwrite(STDOUT, "world-light-smoke: passed\n");
