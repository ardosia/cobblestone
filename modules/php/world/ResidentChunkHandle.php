<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Stable owner-runtime handle to one resident PHP chunk object.
 *
 * It intentionally exposes no lock/guard ceremony because authoritative PHP state has one runtime
 * owner. sameCell() preserves Ardosia's process-local resident-cell identity semantics.
 *
 * @internal
 */
final readonly class ResidentChunkHandle
{
    private ResidentChunkCell $cell;

    public function __construct(Chunk $chunk)
    {
        $this->cell = new ResidentChunkCell($chunk);
    }

    public function position(): ChunkPos
    {
        return $this->cell->position();
    }

    public function cell(): ResidentChunkCell
    {
        return $this->cell;
    }

    public function snapshot(): ChunkSnapshot
    {
        return $this->cell->snapshot();
    }

    public function sameCell(self $other): bool
    {
        return $this->cell->chunk() === $other->cell->chunk();
    }
}
