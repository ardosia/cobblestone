<?php

declare(strict_types=1);

namespace Cobblestone\Server;

use Cobblestone\World\Generator\FlatGenerator;
use Cobblestone\World\Generator\Generator;
use Cobblestone\World\MainChunkSource;
use Cobblestone\World\Mutation\MutationCoordinator;
use Cobblestone\World\Region\RegionMap;
use Cobblestone\World\World;

/** @internal Application composition root for concrete world mechanisms. */
final class WorldFactory
{
    public static function flat(string $name = 'Cobblestone', int $seed = -1): World
    {
        return self::create($name, $seed, FlatGenerator::defaults());
    }

    public static function create(string $name, int $seed, Generator $generator): World
    {
        $chunks = new MainChunkSource($generator, $seed);
        $regions = new RegionMap();
        $mutations = new MutationCoordinator($chunks, $regions);

        return new World($name, $seed, $generator, $chunks, $mutations);
    }
}
