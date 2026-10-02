<?php

declare(strict_types=1);

namespace Cobblestone\World;

use ValueError;

/** Semantic fixed-target light level in range 0..15. */
final readonly class LightLevel
{
    public const MIN_VALUE = 0;
    public const MAX_VALUE = 15;

    public function __construct(public int $value)
    {
        if ($value < self::MIN_VALUE || $value > self::MAX_VALUE) {
            throw new ValueError('fixed-target light level must be in range 0..15');
        }
    }

    public static function min(): self
    {
        return new self(self::MIN_VALUE);
    }

    public static function max(): self
    {
        return new self(self::MAX_VALUE);
    }
}
