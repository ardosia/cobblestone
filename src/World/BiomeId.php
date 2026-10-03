<?php

declare(strict_types=1);

namespace Cobblestone\World;

use ValueError;

final readonly class BiomeId
{
    public const OCEAN = 0;
    public const PLAINS = 1;
    public const DESERT = 2;
    public const EXTREME_HILLS = 3;
    public const FOREST = 4;
    public const TAIGA = 5;
    public const SWAMPLAND = 6;
    public const RIVER = 7;
    public const HELL = 8;

    public function __construct(public int $value)
    {
        if (!BiomeCatalog::supports($value)) {
            throw new ValueError("unsupported MCPE 0.15.10 biome id {$value}");
        }
    }

    public function name(): string
    {
        return BiomeCatalog::name($this->value);
    }

    public function defaultColor(): int
    {
        return BiomeCatalog::defaultColor($this->value);
    }

    public function column(): BiomeColumn
    {
        return BiomeColumn::forId($this);
    }
}
