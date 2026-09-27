<?php

declare(strict_types=1);

namespace Cobblestone\World;

interface ChunkSource
{
    public function get(ChunkPos $position): ?Chunk;

    public function getOrGenerate(ChunkPos $position): Chunk;

    public function put(Chunk $chunk): void;

    /**
     * Compatibility removal surface. Returns the previous chunk only when a safe unload succeeds.
     */
    public function remove(ChunkPos $position): ?Chunk;

    public function unload(ChunkPos $position): ChunkUnloadStatus;

    public function resident(ChunkPos $position, bool $generate = false): ?ResidentChunkHandle;

    public function count(): int;
}
