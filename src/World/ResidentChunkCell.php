<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Shared owner-runtime identity for one resident chunk.
 *
 * Handles acquire/release pins through this cell. Native-backed cells mirror those pins into the
 * Rust WorldStore; PHP-backed cells keep the same semantics locally for parity.
 *
 * @internal
 */
final class ResidentChunkCell
{
    private int $pins = 0;

    public function __construct(private readonly Chunk $chunk)
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

    public function acquire(): void
    {
        $this->chunk->pinBacking();
        ++$this->pins;
    }

    public function release(): void
    {
        if ($this->pins === 0) {
            throw new \LogicException('resident chunk cell is not pinned');
        }

        $this->chunk->unpinBacking();
        --$this->pins;
    }

    public function pinCount(): int
    {
        return $this->pins;
    }

    /** @internal */
    public function chunk(): Chunk
    {
        return $this->chunk;
    }
}
