<?php

declare(strict_types=1);

namespace Cobblestone\Session;

use Cobblestone\World\ChunkLoadPending;
use Cobblestone\World\ChunkLease;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\World;
use LogicException;

/** @internal */
final class ChunkViewPreparation
{
    /** @var array<string, ChunkLease> */
    private array $pins = [];
    private bool $sent = false;

    /**
     * @param list<ChunkPos> $entering
     */
    public function __construct(
        public readonly ChunkPos $fromCenter,
        public readonly int $fromRadius,
        public readonly ChunkPos $toCenter,
        public readonly int $toRadius,
        private readonly array $entering,
    ) {
    }

    public function sameTransition(
        ChunkPos $fromCenter,
        int $fromRadius,
        ChunkPos $toCenter,
        int $toRadius,
    ): bool {
        return $this->fromCenter->x === $fromCenter->x
            && $this->fromCenter->z === $fromCenter->z
            && $this->fromRadius === $fromRadius
            && $this->toCenter->x === $toCenter->x
            && $this->toCenter->z === $toCenter->z
            && $this->toRadius === $toRadius;
    }

    public function prepare(World $world): bool
    {
        foreach ($this->entering as $position) {
            $key = $position->key();
            if (isset($this->pins[$key])) {
                continue;
            }

            try {
                $handle = $world->pinChunk($position, true);
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

    public function sent(): bool
    {
        return $this->sent;
    }

    public function markSent(): void
    {
        if (!$this->prepared()) {
            throw new LogicException('cannot mark an incomplete chunk view preparation as sent');
        }
        $this->sent = true;
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
