<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Stable owner-runtime pinned handle to one resident chunk cell.
 *
 * The handle releases its pin deterministically through release(), with destructor cleanup as a
 * fallback. It exposes no native lock/guard object to gameplay code.
 *
 * @internal
 */
final class ResidentChunkHandle
{
    private bool $released = false;

    public function __construct(private readonly ResidentChunkCell $cell)
    {
        $this->cell->acquire();
    }

    public function __destruct()
    {
        if ($this->released) {
            return;
        }

        try {
            $this->cell->release();
        } catch (\Throwable) {
            // Destructors must not turn shutdown ordering into a fatal error.
        }
        $this->released = true;
    }

    public function release(): void
    {
        if ($this->released) {
            return;
        }

        $this->cell->release();
        $this->released = true;
    }

    public function position(): ChunkPos
    {
        $this->assertActive();

        return $this->cell->position();
    }

    public function cell(): ResidentChunkCell
    {
        $this->assertActive();

        return $this->cell;
    }

    public function snapshot(): ChunkSnapshot
    {
        $this->assertActive();

        return $this->cell->snapshot();
    }

    public function sameCell(self $other): bool
    {
        return $this->cell === $other->cell;
    }

    private function assertActive(): void
    {
        if ($this->released) {
            throw new \LogicException('resident chunk handle was released');
        }
    }
}
