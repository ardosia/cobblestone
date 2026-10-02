<?php

declare(strict_types=1);

namespace Cobblestone\World;

final readonly class ChunkPos
{
    public function __construct(
        public int $x,
        public int $z,
    ) {
    }

    public static function fromBlock(int $x, int $z): self
    {
        return new self(self::floorDiv($x, WorldBounds::CHUNK_EDGE), self::floorDiv($z, WorldBounds::CHUNK_EDGE));
    }

    public static function localCoordinate(int $coordinate): int
    {
        $local = $coordinate % WorldBounds::CHUNK_EDGE;

        return $local < 0 ? $local + WorldBounds::CHUNK_EDGE : $local;
    }

    public function key(): string
    {
        return $this->x . ':' . $this->z;
    }

    private static function floorDiv(int $value, int $divisor): int
    {
        $quotient = intdiv($value, $divisor);
        if ($value < 0 && $value % $divisor !== 0) {
            --$quotient;
        }

        return $quotient;
    }
}
