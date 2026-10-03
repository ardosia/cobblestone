<?php

declare(strict_types=1);

namespace Cobblestone\World;

use ValueError;

/** Immutable MCPE 0.15.10 biome column state: one biome ID plus its independent RGB tint. */
final readonly class BiomeColumn
{
    public function __construct(
        public BiomeId $id,
        public int $color,
    ) {
        if ($color < 0 || $color > 0xffffff) {
            throw new ValueError('fixed-target biome color must be in range 0x000000..0xffffff');
        }
    }

    public static function forId(BiomeId $id): self
    {
        return new self($id, $id->defaultColor());
    }

    public static function fromWord(int $word): self
    {
        if ($word < 0 || $word > 0xffffffff) {
            throw new ValueError('fixed-target biome word must fit unsigned 32 bits');
        }

        return new self(
            new BiomeId(($word >> 24) & 0xff),
            $word & 0xffffff,
        );
    }

    public function word(): int
    {
        return ($this->id->value << 24) | $this->color;
    }

    public function withId(BiomeId $id): self
    {
        return new self($id, $this->color);
    }

    public function withColor(int $color): self
    {
        return new self($this->id, $color);
    }
}
