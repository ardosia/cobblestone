<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Exact dimensions exposed by Minecraft: Windows 10 Edition Beta 0.15.10.
 *
 * Backed values are the protocol-84 StartGame/ChangeDimension wire identifiers.
 */
enum Dimension: int
{
    case Overworld = 0;
    case Nether = 1;

    public function hasSky(): bool
    {
        return $this === self::Overworld;
    }
}
