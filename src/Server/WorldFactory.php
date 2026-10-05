<?php

declare(strict_types=1);

namespace Cobblestone\Server;

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
    public const DEFAULT_COMPACTION_MIN_DEAD_BYTES = 64 * 1024 * 1024;
    public const DEFAULT_COMPACTION_MIN_DEAD_PERCENT = 50;

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
        int $saveWorkers = 2,
        int $loadWorkers = 2,
        int $compactionMinDeadBytes = self::DEFAULT_COMPACTION_MIN_DEAD_BYTES,
        int $compactionMinDeadPercent = self::DEFAULT_COMPACTION_MIN_DEAD_PERCENT,
    ): World {
        if (!NativeWorld::available()) {
            throw new \RuntimeException('persistent Infinite worlds require cobblestone_core_php');
        }

        $creationGenerator = new InfiniteGenerator($seed);
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
                saveWorkers: $saveWorkers,
                loadWorkers: $loadWorkers,
                compactionMinDeadBytes: $compactionMinDeadBytes,
                compactionMinDeadPercent: $compactionMinDeadPercent,
                createDimension: Dimension::Overworld,
            );
            if ($metadata->generatorId !== GeneratorType::Infinite->value) {
                throw new \LogicException(
                    "persistent world generator {$metadata->generatorId} is not Infinite",
                );
            }
            if ($metadata->generatorSettingsVersion !== 1 || $metadata->generatorSettings !== '') {
                throw new \LogicException('persistent Infinite generator settings are unsupported');
            }
            if ($metadata->dimension !== Dimension::Overworld) {
                throw new \LogicException('Infinite generator is only valid for the Overworld');
            }

            $generator = new InfiniteGenerator($metadata->seed);
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
        int $saveWorkers = 2,
        int $loadWorkers = 2,
        int $compactionMinDeadBytes = self::DEFAULT_COMPACTION_MIN_DEAD_BYTES,
        int $compactionMinDeadPercent = self::DEFAULT_COMPACTION_MIN_DEAD_PERCENT,
        Dimension $dimension = Dimension::Overworld,
    ): World {
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
                saveWorkers: $saveWorkers,
                loadWorkers: $loadWorkers,
                compactionMinDeadBytes: $compactionMinDeadBytes,
                compactionMinDeadPercent: $compactionMinDeadPercent,
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
