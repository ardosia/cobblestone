<?php

declare(strict_types=1);

namespace Cobblestone\World;

/** Staged chunk-local terrain replacement independent of a specific terrain revision. */
final class TerrainPatch
{
    /** @var array<int, BlockState> */
    private array $blocks = [];

    /** @var array<int, BiomeId> */
    private array $biomes = [];

    public function block(ChunkTerrain $terrain, int $x, int $y, int $z): ?BlockState
    {
        $key = self::blockKey($x, $y, $z);
        if ($key === null) {
            return null;
        }

        return $this->blocks[$key] ?? $terrain->data()->block($x, $y, $z);
    }

    public function setBlock(
        ChunkTerrain $terrain,
        int $x,
        int $y,
        int $z,
        BlockState $state,
    ): ?BlockState {
        $key = self::blockKey($x, $y, $z);
        if ($key === null) {
            return null;
        }

        $previous = $this->block($terrain, $x, $y, $z);
        if ($previous?->fullId() !== $state->fullId()) {
            $this->blocks[$key] = $state;
        }

        return $previous;
    }

    public function biome(ChunkTerrain $terrain, int $x, int $z): ?BiomeId
    {
        $key = self::columnKey($x, $z);
        if ($key === null) {
            return null;
        }

        return $this->biomes[$key] ?? $terrain->data()->biome($x, $z);
    }

    public function setBiome(
        ChunkTerrain $terrain,
        int $x,
        int $z,
        BiomeId $biome,
    ): ?BiomeId {
        $key = self::columnKey($x, $z);
        if ($key === null) {
            return null;
        }

        $previous = $this->biome($terrain, $x, $z);
        if ($previous?->value !== $biome->value) {
            $this->biomes[$key] = $biome;
        }

        return $previous;
    }

    public function prepare(ChunkTerrain $terrain): PreparedTerrainPatch
    {
        $chunk = $terrain->data();
        $base = $terrain->revision()->value;

        $blocks = array_filter(
            $this->blocks,
            function (BlockState $state, int $key) use ($chunk): bool {
                [$x, $y, $z] = self::decodeBlockKey($key);

                return $state->fullId() !== $chunk->block($x, $y, $z)->fullId();
            },
            ARRAY_FILTER_USE_BOTH,
        );
        $biomes = array_filter(
            $this->biomes,
            function (BiomeId $biome, int $key) use ($chunk): bool {
                [$x, $z] = self::decodeColumnKey($key);

                return $biome->value !== $chunk->biome($x, $z)->value;
            },
            ARRAY_FILTER_USE_BOTH,
        );

        $changed = $blocks !== [] || $biomes !== [];
        if ($changed && $base === PHP_INT_MAX) {
            throw new \OverflowException('chunk terrain revision space exhausted');
        }

        return new PreparedTerrainPatch(
            $base,
            new ChunkRevision($changed ? $base + 1 : $base),
            $changed,
            $blocks,
            $biomes,
        );
    }

    private static function blockKey(int $x, int $y, int $z): ?int
    {
        if (
            !WorldBounds::containsLocal($x)
            || !WorldBounds::containsLocal($z)
            || !WorldBounds::containsY($y)
        ) {
            return null;
        }

        return ($y << 8) | ($z << 4) | $x;
    }

    /** @internal @return array{int, int, int} */
    public static function decodeBlockKey(int $key): array
    {
        return [$key & 0x0f, ($key >> 8) & 0x7f, ($key >> 4) & 0x0f];
    }

    private static function columnKey(int $x, int $z): ?int
    {
        if (!WorldBounds::containsLocal($x) || !WorldBounds::containsLocal($z)) {
            return null;
        }

        return ($z << 4) | $x;
    }

    /** @internal @return array{int, int} */
    public static function decodeColumnKey(int $key): array
    {
        return [$key & 0x0f, ($key >> 4) & 0x0f];
    }
}
