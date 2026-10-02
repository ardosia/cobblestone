<?php

declare(strict_types=1);

namespace Cobblestone\Server;

use Cobblestone\World\Generator\FlatGenerator;
use Cobblestone\World\Generator\Generator;
use Cobblestone\World\Generator\GeneratorType;
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

    public static function flat(string $name = 'Cobblestone', int $seed = -1): World
    {
        return self::create($name, $seed, FlatGenerator::defaults());
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

    public static function create(string $name, int $seed, Generator $generator): World
    {
        $nativeStore = NativeWorld::available() ? NativeWorld::create() : null;

        return self::compose($name, $seed, $generator, $nativeStore);
    }

    private static function compose(
        string $name,
        int $seed,
        Generator $generator,
        ?NativeWorld $nativeStore,
    ): World {
        $chunks = new MainChunkSource($generator, $seed, $nativeStore);
        $regions = new RegionMap();
        $mutations = new MutationCoordinator($chunks, $regions);

        return new World($name, $seed, $generator, $chunks, $mutations, $nativeStore);
    }
}
