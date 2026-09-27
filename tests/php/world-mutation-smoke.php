<?php

declare(strict_types=1);

require __DIR__ . '/bootstrap.php';

use Cobblestone\Server\WorldFactory;
use Cobblestone\World\BlockPos;
use Cobblestone\World\BlockState;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\Generator\FlatGenerator;
use Cobblestone\World\Mutation\WorldMutation;
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

$result = $world->mutate(
    static function (WorldMutation $mutation) use ($first, $second): string {
        mutationExpect($mutation->block($first)->isAir(), 'first staged source was not air');
        $mutation->setBlock($first, new BlockState(5));
        mutationExpect($mutation->block($first)->id === 5, 'staged read did not see first write');

        $mutation->setBlock($second, new BlockState(4));
        mutationExpect($mutation->block($second)->id === 4, 'staged cross-region read did not see write');

        return 'committed';
    },
);

mutationExpect($result->value === 'committed', 'mutation callback result was not preserved');
mutationExpect($result->attempts === 2, 'cross-chunk discovery did not use discard/replay');
mutationExpect(count($result->changedChunks) === 2, 'two changed chunks were not reported');
mutationExpect($firstChunk->revision() === 1, 'first chunk revision did not advance exactly once');
mutationExpect($secondChunk->revision() === 1, 'second chunk revision did not advance exactly once');

$before = $firstChunk->revision();
$noOp = $world->mutate(
    static function (WorldMutation $mutation) use ($first): void {
        $original = $mutation->block($first);
        $mutation->setBlock($first, new BlockState(1));
        $mutation->setBlock($first, $original);
    },
);
mutationExpect(!$noOp->changed(), 'reverted mutation must not report a net change');
mutationExpect($firstChunk->revision() === $before, 'reverted mutation advanced revision');

$compound = $world->mutate(
    static function (WorldMutation $mutation) use ($first): void {
        $mutation->setBlock($first, new BlockState(2));
        $mutation->setSkyLight($first, 15);
        $mutation->setBlockLight($first, 7);
        $mutation->setBlockExtraData($first, 0xbeef);
    },
);
mutationExpect($compound->changed(), 'compound mutation did not report change');
mutationExpect($firstChunk->revision() === $before + 1, 'compound mutation advanced revision more than once');
mutationExpect($world->block($first)->id === 2, 'compound block state did not commit');
mutationExpect($world->skyLight($first) === 15, 'compound sky light did not commit');
mutationExpect($world->blockLight($first) === 7, 'compound block light did not commit');
mutationExpect($world->blockExtraData($first) === 0xbeef, 'compound extra data did not commit');

$regions = new RegionMap();
$negative = $regions->forChunk(new ChunkPos(-1, -9));
mutationExpect($negative->x === -1 && $negative->z === -2, 'negative execution-region mapping mismatch');

fwrite(STDOUT, "world-mutation-smoke: passed\n");