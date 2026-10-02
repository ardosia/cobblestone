<?php

declare(strict_types=1);

namespace Cobblestone\World;

use Cobblestone\Native\World as NativeWorld;

use Closure;
use Cobblestone\World\Generator\Generator;
use Cobblestone\World\Generator\GeneratorType;
use Cobblestone\World\Mutation\MutationCoordinatorInterface;
use ValueError;

final class World implements BlockSource
{
    private BlockPos $spawn;
    private int $time = 0;
    private bool $timeStarted = true;

    public function __construct(
        private readonly string $name,
        private readonly int $seed,
        private readonly Generator $generator,
        private readonly ChunkSource $chunks,
        private readonly MutationCoordinatorInterface $mutations,
        private readonly ?NativeWorld $nativeStore = null,
    ) {
        if ($name === '') {
            throw new ValueError('world name cannot be empty');
        }

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

    /** @internal Mechanism access for world/session implementation code. */
    public function chunks(): ChunkSource
    {
        return $this->chunks;
    }

    /** @internal */
    public function nativeStore(): ?NativeWorld
    {
        return $this->nativeStore;
    }

    public function chunk(ChunkPos $position, bool $generate = true): ?Chunk
    {
        return $generate
            ? $this->chunks->getOrGenerate($position)
            : $this->chunks->get($position);
    }

    public function pinChunk(ChunkPos $position, bool $generate = true): ?ChunkLease
    {
        return $this->chunks->resident($position, $generate);
    }

    /**
     * Keeps one chunk resident for exactly the duration of the callback.
     *
     * @param Closure(Chunk): mixed $operation
     */
    public function withChunk(ChunkPos $position, Closure $operation): mixed
    {
        $lease = $this->pinChunk($position, true)
            ?? throw new \LogicException(
                "chunk {$position->x}:{$position->z} did not become resident",
            );

        try {
            return $operation($lease->chunk());
        } finally {
            $lease->release();
        }
    }

    /** @internal Adopts a chunk already loaded into the authoritative native store. */
    public function adoptNativeChunk(ChunkPos $position): Chunk
    {
        return $this->chunks->adoptNativeResident($position);
    }

    /**
     * Runs one replayable, atomic semantic world edit and returns the callback value.
     *
     * @param Closure(WorldEdit): mixed $operation
     */
    public function edit(Closure $operation): mixed
    {
        return $this->mutations->run($operation)->value;
    }

    public function blockStateId(BlockPos $position): int
    {
        $chunk = $this->chunkForBlock($position);

        return $chunk->blockStateId($position->localX(), $position->y, $position->localZ());
    }

    public function setBlockStateId(BlockPos $position, int $stateId): int
    {
        BlockStateId::assert($stateId);
        $result = $this->mutations->run(
            static fn (WorldEdit $edit): int => $edit->setBlockStateId(
                $position,
                $stateId,
            ),
            [$position->chunk()],
        );

        return $result->value;
    }

    public function block(BlockPos $position): BlockState
    {
        return BlockState::fromId($this->blockStateId($position));
    }

    public function setBlock(BlockPos $position, BlockState $state): BlockState
    {
        return BlockState::fromId($this->setBlockStateId($position, $state->fullId()));
    }

    public function biomeAt(int $x, int $z): BiomeId
    {
        $position = ChunkPos::fromBlock($x, $z);
        $chunk = $this->chunks->getOrGenerate($position);

        return $chunk->biome(ChunkPos::localCoordinate($x), ChunkPos::localCoordinate($z));
    }

    public function setBiomeAt(int $x, int $z, BiomeId $biome): BiomeId
    {
        $chunkPosition = ChunkPos::fromBlock($x, $z);
        $result = $this->mutations->run(
            static fn (WorldEdit $edit): BiomeId => $edit->setBiomeAt($x, $z, $biome),
            [$chunkPosition],
        );

        return $result->value;
    }

    public function skyLight(BlockPos $position): int
    {
        $chunk = $this->chunkForBlock($position);

        return $chunk->skyLight($position->localX(), $position->y, $position->localZ());
    }

    public function setSkyLight(BlockPos $position, int $level): int
    {
        $result = $this->mutations->run(
            static fn (WorldEdit $edit): int => $edit->setSkyLight($position, $level),
            [$position->chunk()],
        );

        return $result->value;
    }

    public function blockLight(BlockPos $position): int
    {
        $chunk = $this->chunkForBlock($position);

        return $chunk->blockLight($position->localX(), $position->y, $position->localZ());
    }

    public function setBlockLight(BlockPos $position, int $level): int
    {
        $result = $this->mutations->run(
            static fn (WorldEdit $edit): int => $edit->setBlockLight($position, $level),
            [$position->chunk()],
        );

        return $result->value;
    }

    public function blockExtraData(BlockPos $position): int
    {
        $chunk = $this->chunkForBlock($position);

        return $chunk->blockExtraData($position->localX(), $position->y, $position->localZ());
    }

    public function setBlockExtraData(BlockPos $position, int $data): int
    {
        $result = $this->mutations->run(
            static fn (WorldEdit $edit): int => $edit->setBlockExtraData($position, $data),
            [$position->chunk()],
        );

        return $result->value;
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
