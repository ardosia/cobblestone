<?php

declare(strict_types=1);

namespace Cobblestone\World;

use ValueError;

/** Monotonic invalidation identity for committed terrain state. */
final readonly class ChunkRevision
{
    public function __construct(public int $value = 0)
    {
        if ($value < 0) {
            throw new ValueError('chunk revision cannot be negative');
        }
    }

    public static function initial(): self
    {
        return new self(0);
    }

    public function checkedNext(): ?self
    {
        return $this->value === PHP_INT_MAX ? null : new self($this->value + 1);
    }
}
