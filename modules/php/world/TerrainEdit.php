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

    public function block(int $x, int $y, int $z): ?BlockState
    {
        return $this->patch->block($this->terrain, $x, $y, $z);
    }

    public function setBlock(int $x, int $y, int $z, BlockState $state): ?BlockState
    {
        return $this->patch->setBlock($this->terrain, $x, $y, $z, $state);
    }

    public function biome(int $x, int $z): ?BiomeId
    {
        return $this->patch->biome($this->terrain, $x, $z);
    }

    public function setBiome(int $x, int $z, BiomeId $biome): ?BiomeId
    {
        return $this->patch->setBiome($this->terrain, $x, $z, $biome);
    }

    public function commit(): TerrainEditResult
    {
        return $this->terrain->commitPrepared($this->patch->prepare($this->terrain));
    }
}
