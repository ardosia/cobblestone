<?php

declare(strict_types=1);

require __DIR__ . '/bootstrap.php';

use Cobblestone\Server\WorldFactory;
use Cobblestone\World\BlockPos;
use Cobblestone\World\BlockState;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\Generator\FlatGenerator;
use Cobblestone\World\WorldEdit;
use Cobblestone\World\Region\RegionMap;

function mutationExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

$world = WorldFactory::create('Mutation Test', 99, FlatGenerator::defaults());
$first = new BlockPos(0, 4, 0);
$second = new BlockPos(128, 4, 0);

$firstChunk = $world->chunk($first->chunk());
$secondChunk = $world->chunk($second->chunk());
mutationExpect($firstChunk !== null && $secondChunk !== null, 'test chunks were not generated');
mutationExpect($firstChunk->revision() === 0, 'generated chunk revision must start at zero');
mutationExpect($secondChunk->revision() === 0, 'generated chunk revision must start at zero');

$attempts = 0;
$result = $world->edit(
    static function (WorldEdit $edit) use ($first, $second, &$attempts): string {
        ++$attempts;
        mutationExpect($edit->block($first)->isAir(), 'first staged source was not air');
        $edit->setBlock($first, new BlockState(5));
        mutationExpect($edit->block($first)->id === 5, 'staged read did not see first write');

        $edit->setBlock($second, new BlockState(4));
        mutationExpect($edit->block($second)->id === 4, 'staged cross-region read did not see write');

        return 'committed';
    },
);

mutationExpect($result === 'committed', 'world edit callback result was not preserved');
mutationExpect($attempts === 2, 'cross-chunk discovery did not use discard/replay');
mutationExpect($firstChunk->revision() === 1, 'first chunk revision did not advance exactly once');
mutationExpect($secondChunk->revision() === 1, 'second chunk revision did not advance exactly once');

$before = $firstChunk->revision();
$beforeLight = $firstChunk->lightRevision()->value;
$world->edit(
    static function (WorldEdit $edit) use ($first): void {
        $original = $edit->block($first);
        $edit->setBlock($first, new BlockState(1));
        $edit->setBlock($first, $original);
    },
);
mutationExpect($firstChunk->revision() === $before, 'reverted mutation advanced terrain revision');
mutationExpect(
    $firstChunk->lightRevision()->value === $beforeLight,
    'reverted mutation advanced light revision',
);

$world->edit(
    static function (WorldEdit $edit) use ($first): void {
        $edit->setBlock($first, new BlockState(2));
        $edit->setSkyLight($first, 15);
        $edit->setBlockLight($first, 7);
        $edit->setBlockExtraData($first, 0xbeef);
    },
);
mutationExpect(
    $firstChunk->revision() === $before + 1,
    'compound mutation did not advance terrain revision exactly once',
);
mutationExpect(
    $firstChunk->lightRevision()->value === $beforeLight + 1,
    'compound mutation did not advance light revision exactly once',
);
mutationExpect($world->block($first)->id === 2, 'compound block state did not commit');
mutationExpect($world->skyLight($first) === 15, 'compound sky light did not commit');
mutationExpect($world->blockLight($first) === 7, 'compound block light did not commit');
mutationExpect($world->blockExtraData($first) === 0xbeef, 'compound extra data did not commit');

$regions = new RegionMap();
$negative = $regions->forChunk(new ChunkPos(-1, -9));
mutationExpect($negative->x === -1 && $negative->z === -2, 'negative execution-region mapping mismatch');

fwrite(STDOUT, "world-mutation-smoke: passed\n");