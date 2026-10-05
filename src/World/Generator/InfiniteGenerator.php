<?php

declare(strict_types=1);

namespace Cobblestone\World\Generator;

use Cobblestone\Native\World as NativeWorld;
use Cobblestone\Native\World\BiomeSource as NativeBiomeSource;
use Cobblestone\Native\World\LoadStatus;
use Cobblestone\World\BlockPos;
use Cobblestone\World\Chunk;
use Cobblestone\World\ChunkLoadPending;
use Cobblestone\World\ChunkPos;
use ValueError;

final class InfiniteGenerator implements Generator
{
    private readonly BlockPos $spawn;

    public function __construct(
        private readonly int $seed,
    ) {
        if ($seed < -0x80000000 || $seed > 0x7fffffff) {
            throw new ValueError('MCPE 0.15.10 Infinite world seed must fit signed 32 bits');
        }

        $spawn = NativeBiomeSource::overworldSpawn($seed);
        // Target LevelData stores LEVEL_SPAWN_HEIGHT and lets Player resolve terrain height later.
        // Cobblestone does not have that Player semantic yet, so keep X/Z exact and use sea level
        // as the safe temporary session-facing Y until that subsystem owns the sentinel behavior.
        $this->spawn = new BlockPos($spawn['x'], 64, $spawn['z']);
    }

    public function name(): string
    {
        return 'infinite';
    }

    public function type(): GeneratorType
    {
        return GeneratorType::Infinite;
    }

    public function settings(): array
    {
        return [];
    }

    public function generate(
        ChunkPos $position,
        int $seed,
        ?NativeWorld $nativeStore = null,
    ): Chunk {
        if ($seed !== $this->seed) {
            throw new \LogicException('Infinite generator seed does not match world seed');
        }
        if ($nativeStore === null) {
            throw new \RuntimeException(
                'Infinite generation requires the cobblestone_core_php native world store',
            );
        }
        if (!$nativeStore->generateInfinite($position, $seed)) {
            throw new ChunkLoadPending($position, LoadStatus::Queued);
        }

        return new Chunk(
            $position,
            nativeStore: $nativeStore,
            nativeResident: true,
        );
    }

    public function populate(Chunk $chunk, int $seed): void
    {
        if ($seed !== $this->seed) {
            throw new \LogicException('Infinite generator seed does not match world seed');
        }
        // Native generation already ran the exact target population/finalizer pipeline.
    }

    public function spawn(): BlockPos
    {
        return $this->spawn;
    }
}
