<?php

declare(strict_types=1);

namespace Cobblestone\World;

/** Mutable convenience wrapper around TerrainPatch for one chunk terrain revision. */
final class TerrainEdit
{
    private readonly TerrainPatch $patch;

    public function __construct(private readonly ChunkTerrain $terrain)
    {
        $this->patch = new TerrainPatch();
    }

    public function blockStateId(int $x, int $y, int $z): ?int
    {
        return $this->patch->blockStateId($this->terrain, $x, $y, $z);
    }

    public function block(int $x, int $y, int $z): ?BlockState
    {
        return $this->patch->block($this->terrain, $x, $y, $z);
    }

    public function setBlockStateId(int $x, int $y, int $z, int $stateId): ?int
    {
        return $this->patch->setBlockStateId($this->terrain, $x, $y, $z, $stateId);
    }

    public function setBlock(int $x, int $y, int $z, BlockState $state): ?BlockState
    {
        return $this->patch->setBlock($this->terrain, $x, $y, $z, $state);
    }

    public function biomeColumn(int $x, int $z): ?BiomeColumn
    {
        return $this->patch->biomeColumn($this->terrain, $x, $z);
    }

    public function biome(int $x, int $z): ?BiomeId
    {
        return $this->patch->biome($this->terrain, $x, $z);
    }

    public function biomeColor(int $x, int $z): ?int
    {
        return $this->patch->biomeColor($this->terrain, $x, $z);
    }

    public function setBiomeColumn(int $x, int $z, BiomeColumn $biome): ?BiomeColumn
    {
        return $this->patch->setBiomeColumn($this->terrain, $x, $z, $biome);
    }

    public function setBiome(int $x, int $z, BiomeId $biome): ?BiomeId
    {
        return $this->patch->setBiome($this->terrain, $x, $z, $biome);
    }

    public function setBiomeColor(int $x, int $z, int $color): ?int
    {
        return $this->patch->setBiomeColor($this->terrain, $x, $z, $color);
    }

    public function commit(): bool
    {
        return $this->terrain->commitPrepared($this->patch->prepare($this->terrain));
    }
}
