<?php

declare(strict_types=1);

namespace Cobblestone\World;

use Cobblestone\World\Generator\Generator;
use Cobblestone\World\Generator\GeneratorType;
use ValueError;

final class World implements BlockSource
{
    private readonly ChunkSource $chunks;
    private BlockPos $spawn;
    private int $time = 0;
    private bool $timeStarted = true;

    public function __construct(
        private readonly string $name,
        private readonly int $seed,
        private readonly Generator $generator,
        ?ChunkSource $chunks = null,
    ) {
        if ($name === '') {
            throw new ValueError('world name cannot be empty');
        }

        $this->chunks = $chunks ?? new MainChunkSource($generator, $seed);
        $this->spawn = $generator->spawn();
    }

    public function name(): string
    {
        return $this->name;
    }

    public function seed(): int
    {
        return $this->seed;
    }

    public function generator(): Generator
    {
        return $this->generator;
    }

    public function generatorType(): GeneratorType
    {
        return $this->generator->type();
    }

    public function chunks(): ChunkSource
    {
        return $this->chunks;
    }

    public function chunk(ChunkPos $position, bool $generate = true): ?Chunk
    {
        return $generate
            ? $this->chunks->getOrGenerate($position)
            : $this->chunks->get($position);
    }

    public function block(BlockPos $position): BlockState
    {
        $chunk = $this->chunkForBlock($position);

        return $chunk->block($position->localX(), $position->y, $position->localZ());
    }

    public function setBlock(BlockPos $position, BlockState $state): BlockState
    {
        $chunk = $this->chunkForBlock($position);

        return $chunk->setBlock($position->localX(), $position->y, $position->localZ(), $state);
    }

    public function biomeAt(int $x, int $z): BiomeId
    {
        $position = ChunkPos::fromBlock($x, $z);
        $chunk = $this->chunks->getOrGenerate($position);

        return $chunk->biome(ChunkPos::localCoordinate($x), ChunkPos::localCoordinate($z));
    }

    public function skyLight(BlockPos $position): int
    {
        $chunk = $this->chunkForBlock($position);

        return $chunk->skyLight($position->localX(), $position->y, $position->localZ());
    }

    public function blockLight(BlockPos $position): int
    {
        $chunk = $this->chunkForBlock($position);

        return $chunk->blockLight($position->localX(), $position->y, $position->localZ());
    }

    public function spawn(): BlockPos
    {
        return $this->spawn;
    }

    public function setSpawn(BlockPos $spawn): void
    {
        if (!$spawn->isInsideWorld()) {
            throw new ValueError('world spawn y must be in fixed-target range 0..127');
        }

        $this->spawn = $spawn;
    }

    public function time(): int
    {
        return $this->time;
    }

    public function setTime(int $time): void
    {
        $this->time = $time;
    }

    public function isTimeStarted(): bool
    {
        return $this->timeStarted;
    }

    public function setTimeStarted(bool $started): void
    {
        $this->timeStarted = $started;
    }

    private function chunkForBlock(BlockPos $position): Chunk
    {
        if (!$position->isInsideWorld()) {
            throw new ValueError('block y must be in fixed-target range 0..127');
        }

        return $this->chunks->getOrGenerate($position->chunk());
    }
}
