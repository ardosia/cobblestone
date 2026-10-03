<?php

declare(strict_types=1);

require dirname(__DIR__) . '/bootstrap.php';

use Cobblestone\Server\WorldFactory;
use Cobblestone\World\Generator\Generator;
use Cobblestone\World\MainChunkSource;
use Cobblestone\World\Mutation\MutationCoordinator;
use Cobblestone\World\Region\RegionMap;
use Cobblestone\World\World;

function compositionExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

$root = dirname(__DIR__, 3);
$composer = json_decode(
    (string) file_get_contents($root . '/composer.json'),
    true,
    flags: JSON_THROW_ON_ERROR,
);

compositionExpect(
    ($composer['autoload']['psr-4']['Cobblestone\\'] ?? null) === 'src/',
    'root Composer package must map Cobblestone\\ to src/',
);
compositionExpect(
    ($composer['autoload']['files'] ?? []) === ['src/Command/functions.php'],
    'command helper functions must autoload from src/',
);
compositionExpect(
    !is_dir($root . '/modules/php'),
    'legacy PHP package workspace must be removed',
);
foreach (['Generator', 'Light', 'Mutation'] as $domain) {
    compositionExpect(
        is_dir($root . "/src/World/{$domain}"),
        "World {$domain} namespace must live under src/World",
    );
}

$constructor = new ReflectionMethod(World::class, '__construct');
$types = array_map(
    static fn (ReflectionParameter $parameter): ?string => $parameter->getType() instanceof ReflectionNamedType
        ? $parameter->getType()->getName()
        : null,
    $constructor->getParameters(),
);
compositionExpect(in_array(Generator::class, $types, true), 'World must accept the generator contract');
compositionExpect(
    in_array(MainChunkSource::class, $types, true),
    'World must own the concrete main chunk source',
);
compositionExpect(
    in_array(MutationCoordinator::class, $types, true),
    'World must own the concrete mutation coordinator',
);

$mutationConstructor = new ReflectionMethod(MutationCoordinator::class, '__construct');
$mutationTypes = array_map(
    static fn (ReflectionParameter $parameter): ?string => $parameter->getType() instanceof ReflectionNamedType
        ? $parameter->getType()->getName()
        : null,
    $mutationConstructor->getParameters(),
);
compositionExpect(
    in_array(MainChunkSource::class, $mutationTypes, true),
    'mutation coordinator must use the main chunk source',
);
compositionExpect(
    in_array(RegionMap::class, $mutationTypes, true),
    'mutation coordinator must use the concrete region map',
);

$world = WorldFactory::flat('Composition Smoke', 123);
compositionExpect($world->name() === 'Composition Smoke', 'server world factory did not compose world name');
compositionExpect($world->seed() === 123, 'server world factory did not compose world seed');

fwrite(STDOUT, "world-composition-smoke: passed\n");
