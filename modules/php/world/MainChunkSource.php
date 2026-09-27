<?php

declare(strict_types=1);

namespace Cobblestone\World;

use Cobblestone\World\Generator\Generator;

final class MainChunkSource implements ChunkSource
{
    /** @var array<string, Chunk> */
    private array $chunks = [];

    public function __construct(
        private readonly Generator $generator,
        private readonly int $seed,
        private readonly ?NativeWorldStore $nativeStore = null,
    ) {
    }

    public function get(ChunkPos $position): ?Chunk
    {
        return $this->chunks[$position->key()] ?? null;
    }

    public function getOrGenerate(ChunkPos $position): Chunk
    {
        $existing = $this->get($position);
        if ($existing !== null) {
            return $existing;
        }

        $chunk = $this->generator->generate($position, $this->seed, $this->nativeStore);
        if ($chunk->position()->x !== $position->x || $chunk->position()->z !== $position->z) {
            throw new \LogicException('world generator returned a chunk for the wrong position');
        }

        $chunk->markGenerated();
        $this->generator->populate($chunk, $this->seed);
        $chunk->markPopulated();
        $this->put($chunk);

        return $chunk;
    }

    /** @internal */
    public function nativeStore(): ?NativeWorldStore
    {
        return $this->nativeStore;
    }

    public function put(Chunk $chunk): void
    {
        $this->chunks[$chunk->position()->key()] = $chunk;
    }

    public function remove(ChunkPos $position): ?Chunk
    {
        $key = $position->key();
        $chunk = $this->chunks[$key] ?? null;
        unset($this->chunks[$key]);

        return $chunk;
    }

    public function count(): int
    {
        return count($this->chunks);
    }

    public function resident(ChunkPos $position, bool $generate = false): ?ResidentChunkHandle
    {
        $chunk = $generate ? $this->getOrGenerate($position) : $this->get($position);

        return $chunk === null ? null : new ResidentChunkHandle($chunk);
    }
}
