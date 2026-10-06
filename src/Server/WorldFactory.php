<?php

declare(strict_types=1);

namespace Cobblestone\Server;

use Cobblestone\Config\StorageConfig;
use Cobblestone\World\BlockPos;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\Dimension;
use Cobblestone\World\Generator\FlatGenerator;
use Cobblestone\World\Generator\Generator;
use Cobblestone\World\Generator\GeneratorType;
use Cobblestone\World\Generator\InfiniteGenerator;
use Cobblestone\World\MainChunkSource;
use Cobblestone\World\Mutation\MutationCoordinator;
use Cobblestone\Native\World as NativeWorld;
use Cobblestone\World\Region\RegionMap;
use Cobblestone\World\World;
use Throwable;

/** @internal Application composition root for concrete world mechanisms. */
final class WorldFactory
{
    public static function flat(
        string $name = 'Cobblestone',
        int $seed = -1,
        Dimension $dimension = Dimension::Overworld,
    ): World {
        return self::create($name, $seed, FlatGenerator::defaults(), $dimension);
    }

    public static function infinite(
        string $name = 'Cobblestone',
        int $seed = -1,
    ): World {
        if (!NativeWorld::available()) {
            throw new \RuntimeException('Infinite worlds require cobblestone_core_php');
        }

        return self::create($name, $seed, new InfiniteGenerator($seed), Dimension::Overworld);
    }

    public static function persistentInfinite(
        string $root,
        string $name = 'Cobblestone',
        int $seed = -1,
        ?StorageConfig $storage = null,
    ): World {
        $storage ??= new StorageConfig();
        if (!NativeWorld::available()) {
            throw new \RuntimeException('persistent Infinite worlds require cobblestone_core_php');
        }

        $creationGenerator = InfiniteGenerator::provisional($seed);
        $nativeStore = NativeWorld::create();
        try {
            $metadata = $nativeStore->attachStorage(
                $root,
                $name,
                $seed,
                GeneratorType::Infinite->value,
                1,
                '',
                $creationGenerator->spawn(),
                saveWorkers: $storage->saveWorkers,
                loadWorkers: $storage->loadWorkers,
                compactionMinDeadBytes: $storage->compactionMinDeadBytes,
                compactionMinDeadPercent: $storage->compactionMinDeadPercent,
                createDimension: Dimension::Overworld,
            );
            if ($metadata->generatorId !== GeneratorType::Infinite->value) {
                throw new \LogicException(
                    "persistent world generator {$metadata->generatorId} is not Infinite",
                );
            }
            if ($metadata->generatorSettings !== '') {
                throw new \LogicException('persistent Infinite generator settings are unsupported');
            }
            if ($metadata->dimension !== Dimension::Overworld) {
                throw new \LogicException('Infinite generator is only valid for the Overworld');
            }

            if ($metadata->generatorSettingsVersion >= 1
                && $metadata->generatorSettingsVersion <= 3
            ) {
                // v1-v3 may contain the provisional/incorrect pre-0.15.10 spawn X/Z. Recompute
                // BiomeSource's fixed-target X/Z from the authoritative stored seed before
                // generating the spawn view; never use legacy metadata as the search anchor.
                $targetSpawn = InfiniteGenerator::provisional($metadata->seed)->spawn();
                $resolvedSpawn = self::resolvePersistentInfiniteSpawn(
                    $nativeStore,
                    $metadata->seed,
                    $targetSpawn,
                );
                $metadata = $nativeStore->migrateInfiniteSpawn($resolvedSpawn);
                $generator = new InfiniteGenerator($metadata->seed, $metadata->spawn);
            } elseif ($metadata->generatorSettingsVersion === 4) {
                $generator = new InfiniteGenerator($metadata->seed, $metadata->spawn);
            } else {
                throw new \LogicException(
                    "persistent Infinite generator settings version {$metadata->generatorSettingsVersion} is unsupported",
                );
            }

            $world = self::compose(
                $metadata->name,
                $metadata->seed,
                $metadata->dimension,
                $generator,
                $nativeStore,
            );
            $world->setSpawn($metadata->spawn);
            $world->setTime($metadata->time);
            $world->setTimeStarted($metadata->timeRunning);

            return $world;
        } catch (Throwable $error) {
            try {
                $nativeStore->destroy();
            } catch (Throwable) {
            }
            throw $error;
        }
    }

    public static function persistentFlat(
        string $root,
        string $name = 'Cobblestone',
        int $seed = -1,
        ?string $preset = null,
        ?StorageConfig $storage = null,
        Dimension $dimension = Dimension::Overworld,
    ): World {
        $storage ??= new StorageConfig();
        if (!NativeWorld::available()) {
            throw new \RuntimeException('persistent worlds require cobblestone_core_php native world storage');
        }

        $creationGenerator = $preset === null
            ? FlatGenerator::defaults()
            : FlatGenerator::fromPreset($preset);
        $settings = $creationGenerator->settings();
        $creationPreset = $settings['preset'] ?? null;
        if (!is_string($creationPreset) || $creationPreset === '') {
            throw new \LogicException('Flat generator did not expose its canonical preset');
        }

        $nativeStore = NativeWorld::create();
        try {
            $metadata = $nativeStore->attachStorage(
                $root,
                $name,
                $seed,
                GeneratorType::Flat->value,
                1,
                $creationPreset,
                $creationGenerator->spawn(),
                saveWorkers: $storage->saveWorkers,
                loadWorkers: $storage->loadWorkers,
                compactionMinDeadBytes: $storage->compactionMinDeadBytes,
                compactionMinDeadPercent: $storage->compactionMinDeadPercent,
                createDimension: $dimension,
            );
            if ($metadata->generatorId !== GeneratorType::Flat->value) {
                throw new \LogicException(
                    "persistent world generator {$metadata->generatorId} is unsupported",
                );
            }
            if ($metadata->generatorSettingsVersion !== 1) {
                throw new \LogicException(
                    "persistent Flat generator settings version {$metadata->generatorSettingsVersion} is unsupported",
                );
            }

            $generator = FlatGenerator::fromPreset($metadata->generatorSettings);
            $world = self::compose(
                $metadata->name,
                $metadata->seed,
                $metadata->dimension,
                $generator,
                $nativeStore,
            );
            $world->setSpawn($metadata->spawn);
            $world->setTime($metadata->time);
            $world->setTimeStarted($metadata->timeRunning);

            return $world;
        } catch (Throwable $error) {
            try {
                $nativeStore->destroy();
            } catch (Throwable) {
            }
            throw $error;
        }
    }

    public static function create(
        string $name,
        int $seed,
        Generator $generator,
        Dimension $dimension = Dimension::Overworld,
    ): World {
        $nativeStore = NativeWorld::available() ? NativeWorld::create() : null;

        return self::compose($name, $seed, $dimension, $generator, $nativeStore);
    }

    private static function resolvePersistentInfiniteSpawn(
        NativeWorld $nativeStore,
        int $seed,
        BlockPos $provisionalSpawn,
    ): BlockPos {
        $center = $provisionalSpawn->chunk();

        for ($chunkX = $center->x - 2; $chunkX <= $center->x + 2; ++$chunkX) {
            for ($chunkZ = $center->z - 2; $chunkZ <= $center->z + 2; ++$chunkZ) {
                $position = new ChunkPos($chunkX, $chunkZ);
                $generated = false;
                for ($attempt = 0; $attempt < 10_000; ++$attempt) {
                    if ($nativeStore->generateInfinite($position, $seed)) {
                        $generated = true;
                        break;
                    }
                    // generateInfinite() polls its own dependency-load completions. Do not run the
                    // full storage tick here: that may publish saves while adjacent loads are
                    // still probing the same new region.
                    usleep(1_000);
                }
                if (!$generated) {
                    throw new \RuntimeException(
                        "persistent Infinite spawn view did not resolve chunk {$chunkX}:{$chunkZ}",
                    );
                }
            }
        }

        // Publish the resolved spawn only after the exact chunks it was derived from are durable.
        $nativeStore->flushStorage();

        return $nativeStore->resolveOverworldSpawn(
            $provisionalSpawn->x,
            $provisionalSpawn->z,
        );
    }

    private static function compose(
        string $name,
        int $seed,
        Dimension $dimension,
        Generator $generator,
        ?NativeWorld $nativeStore,
    ): World {
        $chunks = new MainChunkSource($generator, $seed, $nativeStore);
        $regions = new RegionMap();
        $mutations = new MutationCoordinator($chunks, $regions);

        return new World($name, $seed, $dimension, $generator, $chunks, $mutations, $nativeStore);
    }
}
