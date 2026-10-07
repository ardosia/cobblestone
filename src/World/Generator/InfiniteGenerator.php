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
        ?BlockPos $spawn = null,
    ) {
        if ($seed < -0x80000000 || $seed > 0x7fffffff) {
            throw new ValueError('MCPE 0.15.10 Infinite world seed must fit signed 32 bits');
        }

        if ($spawn === null) {
            $resolved = NativeBiomeSource::overworldInitialSpawn($seed);
            $spawn = new BlockPos($resolved['x'], $resolved['y'], $resolved['z']);
        }
        $this->spawn = $spawn;
    }

    /** @internal Legacy-v1 persistent metadata creation boundary before safe-spawn migration. */
    public static function provisional(int $seed): self
    {
        $spawn = NativeBiomeSource::overworldSpawn($seed);

        return new self($seed, new BlockPos($spawn['x'], 64, $spawn['z']));
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
        NativeWorld $nativeStore,
    ): Chunk {
        if ($seed !== $this->seed) {
            throw new \LogicException('Infinite generator seed does not match world seed');
        }
        if (!$nativeStore->generateInfinite($position, $seed)) {
            throw new ChunkLoadPending($position, LoadStatus::Queued);
        }

        return new Chunk(
            $position,
            $nativeStore,
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
