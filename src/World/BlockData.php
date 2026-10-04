<?php

declare(strict_types=1);

namespace Cobblestone\World;

use ValueError;

/** Exact legacy 4-bit block data carried by protocol-84 block states. */
enum BlockData: int
{
    case Zero = 0;
    case One = 1;
    case Two = 2;
    case Three = 3;
    case Four = 4;
    case Five = 5;
    case Six = 6;
    case Seven = 7;
    case Eight = 8;
    case Nine = 9;
    case Ten = 10;
    case Eleven = 11;
    case Twelve = 12;
    case Thirteen = 13;
    case Fourteen = 14;
    case Fifteen = 15;

    public static function of(int $value): self
    {
        return self::tryFrom($value)
            ?? throw new ValueError("fixed-target block data must be in range 0..15; got {$value}");
    }
}
