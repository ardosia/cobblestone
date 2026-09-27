<?php

declare(strict_types=1);

namespace Cobblestone\World;

use ValueError;

final readonly class BlockState
{
    public function __construct(
        public int $id,
        public int $data = 0,
    ) {
        if ($id < 0 || $id > 0xff) {
            throw new ValueError('fixed-target block id must be in range 0..255');
        }
        if ($data < 0 || $data > 0x0f) {
            throw new ValueError('fixed-target block data must be in range 0..15');
        }
    }

    public static function air(): self
    {
        return new self(0);
    }

    public function fullId(): int
    {
        return ($this->id << 4) | $this->data;
    }

    public function isAir(): bool
    {
        return $this->id === 0;
    }
}
