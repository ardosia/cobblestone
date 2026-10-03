<?php

declare(strict_types=1);

require dirname(__DIR__) . '/bootstrap.php';

use Cobblestone\Server\WorldFactory;
use Cobblestone\World\BiomeId;
use Cobblestone\World\BlockState;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\ChunkUnloadStatus;
use Cobblestone\World\LightLevel;
use Cobblestone\World\SectionY;
use Cobblestone\World\TerrainPatch;

function parityExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

parityExpect(SectionY::fromBlockY(-1) === null, 'SectionY accepted negative y');
parityExpect(SectionY::fromBlockY(0)?->minBlockY() === 0, 'SectionY y=0 mismatch');
parityExpect(SectionY::fromBlockY(31)?->value === 1, 'SectionY y=31 mismatch');
parityExpect(SectionY::fromBlockY(127)?->maxBlockY() === 127, 'SectionY y=127 mismatch');
parityExpect(SectionY::fromBlockY(128) === null, 'SectionY accepted y=128');

$world = WorldFactory::flat('World Parity Smoke', 77);
$position = new ChunkPos(0, 0);
$chunk = $world->chunk($position, true);
parityExpect($chunk !== null, 'generated chunk missing');

$firstHandle = $world->pinChunk($position, false);
$secondHandle = $world->pinChunk($position, false);
parityExpect($firstHandle !== null && $secondHandle !== null, 'chunk lease missing');
parityExpect($firstHandle->sameChunk($secondHandle), 'same resident chunk lost cell identity');

$terrain = $chunk->terrain();
parityExpect($terrain->revision()->value === 0, 'terrain revision did not start at zero');
$edit = $terrain->edit();
parityExpect(
    $edit->setBlock(1, 20, 1, new BlockState(1))?->isAir() === true,
    'terrain edit previous block mismatch',
);
$changed = $edit->commit();
parityExpect($changed && $terrain->revision()->value === 1, 'terrain edit revision mismatch');
parityExpect($chunk->block(1, 20, 1)->id === 1, 'terrain edit did not commit block');

$noOp = $terrain->edit();
$noOp->setBlock(1, 20, 1, new BlockState(1));
$noOpChanged = $noOp->commit();
parityExpect(!$noOpChanged, 'terrain no-op reported a change');
parityExpect($terrain->revision()->value === 1, 'terrain no-op advanced revision');

$reverted = $terrain->edit();
$original = $reverted->block(1, 20, 1);
$reverted->setBlock(1, 20, 1, new BlockState(2));
$reverted->setBlock(1, 20, 1, $original);
$revertedChanged = $reverted->commit();
parityExpect(!$revertedChanged, 'reverted terrain edit reported a change');
parityExpect($terrain->revision()->value === 1, 'reverted terrain edit advanced revision');

$patch = new TerrainPatch();
$patch->setBiome($terrain, 3, 4, new BiomeId(2));
$prepared = $patch->prepare($terrain);
parityExpect(
    $prepared->changed && $prepared->revision->value === 2,
    'prepared terrain patch revision mismatch',
);
$preparedChanged = $terrain->commitPrepared($prepared);
parityExpect($preparedChanged, 'prepared terrain patch did not commit');
parityExpect($chunk->biome(3, 4)->value === 2, 'prepared terrain biome did not commit');

$light = $chunk->light();
parityExpect($light->revision()->value === 0, 'light revision did not start at zero');
$beforeLight = $light->snapshot();
$lightEdit = $light->edit();
$lightEdit->setBlock(2, 20, 2, new LightLevel(5));
$lightChanged = $lightEdit->commit();
parityExpect(
    $lightChanged && $light->revision()->value === 1,
    'light edit revision mismatch',
);
parityExpect($light->block(2, 20, 2)?->value === 5, 'light edit did not commit');
parityExpect($beforeLight->block(2, 20, 2)?->value === 0, 'light snapshot mutated after commit');

$lightNoOp = $light->edit();
$lightNoOp->setBlock(2, 20, 2, new LightLevel(5));
parityExpect(!$lightNoOp->commit(), 'light no-op reported change');
parityExpect($light->revision()->value === 1, 'light no-op advanced revision');

$snapshot = $firstHandle->snapshot();
parityExpect(
    $snapshot->position()->x === 0 && $snapshot->position()->z === 0,
    'chunk snapshot position mismatch',
);
parityExpect($snapshot->revision()->value === 2, 'chunk snapshot terrain revision mismatch');
parityExpect($snapshot->light()->revision->value === 1, 'chunk snapshot light revision mismatch');

parityExpect($chunk->isDirty(), 'newly generated/mutated chunk must be dirty');
parityExpect(
    $world->chunks()->unload($position) === ChunkUnloadStatus::Pinned,
    'chunk leases did not block chunk unload',
);
$firstHandle->release();
$secondHandle->release();
parityExpect(
    $world->chunks()->unload($position) === ChunkUnloadStatus::Dirty,
    'dirty chunk did not block safe unload',
);

$chunk->markCurrentStatePersisted();
parityExpect(!$chunk->isDirty(), 'persisted chunk remained dirty');
parityExpect(
    $world->chunks()->unload($position) === ChunkUnloadStatus::Unloaded,
    'clean unpinned chunk did not unload',
);
$replacement = $world->chunk($position, true);
$replacementHandle = $world->pinChunk($position, false);
parityExpect($replacement !== null && $replacementHandle !== null, 'replacement chunk missing');
parityExpect(
    !$firstHandle->sameChunk($replacementHandle),
    'unload/reload reused resident cell identity',
);
$replacementHandle->release();

fwrite(STDOUT, "world-parity-smoke: passed\n");
