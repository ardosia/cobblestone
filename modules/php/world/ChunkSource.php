<?php

declare(strict_types=1);

namespace Cobblestone\World;

interface ChunkSource
{
    public function get(ChunkPos $position): ?Chunk;

    public function getOrGenerate(ChunkPos $position): Chunk;

    /** @internal Adopts a chunk already resident in the authoritative native store. */
    public function adoptNativeResident(ChunkPos $position): Chunk;

    public function put(Chunk $chunk): void;

    public function unload(ChunkPos $position): ChunkUnloadStatus;

    /**
     * Inspects at most $budget resident entries and evicts only clean, unpinned chunks.
     *
     * @return int Number of resident facades removed.
     */
    public function evictCleanUnpinned(int $budget): int;

    public function resident(ChunkPos $position, bool $generate = false): ?ChunkLease;

    public function count(): int;
}
