<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Owner-runtime resident chunk cell.
 *
 * Rust lock guards collapse to direct owner-local access in PHP; terrain/light remain separate
 * semantic facades and snapshot capture is immutable.
 *
 * @internal
 */
final readonly class ResidentChunkCell
{
    public function __construct(private Chunk $chunk)
    {
    }

    public function position(): ChunkPos
    {
        return $this->chunk->position();
    }

    public function terrain(): ChunkTerrain
    {
        return $this->chunk->terrain();
    }

    public function light(): ChunkLight
    {
        return $this->chunk->light();
    }

    public function snapshot(): ChunkSnapshot
    {
        return $this->chunk->snapshot();
    }

    /** @internal */
    public function chunk(): Chunk
    {
        return $this->chunk;
    }
}
