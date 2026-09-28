<?php

declare(strict_types=1);

require __DIR__ . '/bootstrap.php';

use Cobblestone\Server\WorldFactory;
use Cobblestone\World\Generator\Generator;
use Cobblestone\World\Mutation\MutationCoordinatorInterface;
use Cobblestone\World\World;

function compositionExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

$root = dirname(__DIR__, 2);
$world = json_decode(
    (string) file_get_contents($root . '/modules/php/world/composer.json'),
    true,
    flags: JSON_THROW_ON_ERROR,
);
compositionExpect(
    array_filter(
        array_keys($world['require'] ?? []),
        static fn (string $name): bool => str_starts_with($name, 'ardosia/cobblestone-'),
    ) === [],
    'base world package must not depend on sibling Cobblestone packages',
);

foreach (['world-generation', 'world-light', 'world-mutation'] as $package) {
    $manifest = json_decode(
        (string) file_get_contents($root . "/modules/php/{$package}/composer.json"),
        true,
        flags: JSON_THROW_ON_ERROR,
    );
    $cobblestone = array_values(array_filter(
        array_keys($manifest['require'] ?? []),
        static fn (string $name): bool => str_starts_with($name, 'ardosia/cobblestone-'),
    ));
    compositionExpect(
        $cobblestone === ['ardosia/cobblestone-world'],
        "{$package} must depend one-way on the base world package only",
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
    in_array(MutationCoordinatorInterface::class, $types, true),
    'World must accept the mutation coordinator contract',
);
compositionExpect(
    !in_array(Cobblestone\World\Mutation\MutationCoordinator::class, $types, true),
    'World constructor must not depend on the concrete mutation coordinator',
);
compositionExpect(
    !in_array(Cobblestone\World\Region\RegionMap::class, $types, true),
    'World constructor must not depend on the concrete region map',
);

$world = WorldFactory::flat('Composition Smoke', 123);
compositionExpect($world->name() === 'Composition Smoke', 'server world factory did not compose world name');
compositionExpect($world->seed() === 123, 'server world factory did not compose world seed');

fwrite(STDOUT, "world-composition-smoke: passed\n");
