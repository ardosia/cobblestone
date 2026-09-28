<?php

declare(strict_types=1);

namespace Cobblestone\Session;

use Cobblestone\World\ChunkLoadPending;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\ResidentChunkHandle;
use Cobblestone\World\World;
use LogicException;

/** @internal */
final class ChunkViewPreparation
{
    /** @var array<string, ResidentChunkHandle> */
    private array $pins = [];

    /**
     * @param list<ChunkPos> $entering
     */
    public function __construct(
        public readonly ChunkPos $fromCenter,
        public readonly ChunkPos $toCenter,
        private readonly array $entering,
    ) {
    }

    public function sameTransition(ChunkPos $fromCenter, ChunkPos $toCenter): bool
    {
        return $this->fromCenter->x === $fromCenter->x
            && $this->fromCenter->z === $fromCenter->z
            && $this->toCenter->x === $toCenter->x
            && $this->toCenter->z === $toCenter->z;
    }

    public function prepare(World $world): bool
    {
        foreach ($this->entering as $position) {
            $key = $position->key();
            if (isset($this->pins[$key])) {
                continue;
            }

            try {
                $handle = $world->residentChunk($position, true);
            } catch (ChunkLoadPending) {
                continue;
            }

            if ($handle === null) {
                throw new LogicException(
                    "entering chunk {$position->x}:{$position->z} did not become resident",
                );
            }
            $this->pins[$key] = $handle;
        }

        return count($this->pins) === count($this->entering);
    }

    public function prepared(): bool
    {
        return count($this->pins) === count($this->entering);
    }

    public function release(): void
    {
        $failure = null;
        foreach ($this->pins as $handle) {
            try {
                $handle->release();
            } catch (\Throwable $error) {
                $failure ??= $error;
            }
        }
        $this->pins = [];

        if ($failure !== null) {
            throw $failure;
        }
    }

    public function __destruct()
    {
        try {
            $this->release();
        } catch (\Throwable) {
            // Destructors must not turn shutdown ordering into a fatal error.
        }
    }
}
