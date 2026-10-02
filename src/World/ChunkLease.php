<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Deterministic owner-runtime pin on one resident chunk.
 *
 * release() is the correctness path. Destructor cleanup exists only as a shutdown fallback.
 */
final class ChunkLease
{
    private bool $released = false;

    /** @internal */
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

    public function chunk(): Chunk
    {
        $this->assertActive();

        return $this->cell->chunk();
    }

    public function position(): ChunkPos
    {
        return $this->chunk()->position();
    }

    public function snapshot(): ChunkSnapshot
    {
        return $this->chunk()->snapshot();
    }

    public function sameChunk(self $other): bool
    {
        return $this->cell === $other->cell;
    }

    private function assertActive(): void
    {
        if ($this->released) {
            throw new \LogicException('chunk lease was released');
        }
    }
}
