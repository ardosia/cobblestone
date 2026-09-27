<?php

declare(strict_types=1);

namespace Cobblestone\World;

interface ChunkSource
{
    public function get(ChunkPos $position): ?Chunk;

    public function getOrGenerate(ChunkPos $position): Chunk;

    public function put(Chunk $chunk): void;

    public function remove(ChunkPos $position): ?Chunk;

    public function count(): int;
}
