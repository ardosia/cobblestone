<?php

declare(strict_types=1);

namespace Cobblestone\World;

/** Staged chunk-local terrain replacement independent of a specific terrain revision. */
final class TerrainPatch
{
    /** @var array<int, int> scalar BlockStateId tokens */
    private array $blocks = [];

    /** @var array<int, BiomeColumn> */
    private array $biomes = [];

    public function blockStateId(ChunkTerrain $terrain, int $x, int $y, int $z): ?int
    {
        $key = self::blockKey($x, $y, $z);
        if ($key === null) {
            return null;
        }

        return $this->blocks[$key] ?? $terrain->data()->blockStateId($x, $y, $z);
    }

    public function block(ChunkTerrain $terrain, int $x, int $y, int $z): ?BlockState
    {
        $stateId = $this->blockStateId($terrain, $x, $y, $z);

        return $stateId === null ? null : BlockState::fromId($stateId);
    }

    public function setBlockStateId(
        ChunkTerrain $terrain,
        int $x,
        int $y,
        int $z,
        int $stateId,
    ): ?int {
        BlockStateId::assert($stateId);
        $key = self::blockKey($x, $y, $z);
        if ($key === null) {
            return null;
        }

        $previous = $this->blockStateId($terrain, $x, $y, $z);
        if ($previous !== $stateId) {
            $this->blocks[$key] = $stateId;
        }

        return $previous;
    }

    public function setBlock(
        ChunkTerrain $terrain,
        int $x,
        int $y,
        int $z,
        BlockState $state,
    ): ?BlockState {
        $previous = $this->setBlockStateId($terrain, $x, $y, $z, $state->fullId());

        return $previous === null ? null : BlockState::fromId($previous);
    }

    public function biomeColumn(ChunkTerrain $terrain, int $x, int $z): ?BiomeColumn
    {
        $key = self::columnKey($x, $z);
        if ($key === null) {
            return null;
        }

        return $this->biomes[$key] ?? $terrain->data()->biomeColumn($x, $z);
    }

    public function biome(ChunkTerrain $terrain, int $x, int $z): ?BiomeId
    {
        return $this->biomeColumn($terrain, $x, $z)?->id;
    }

    public function biomeColor(ChunkTerrain $terrain, int $x, int $z): ?int
    {
        return $this->biomeColumn($terrain, $x, $z)?->color;
    }

    public function setBiomeColumn(
        ChunkTerrain $terrain,
        int $x,
        int $z,
        BiomeColumn $biome,
    ): ?BiomeColumn {
        $key = self::columnKey($x, $z);
        if ($key === null) {
            return null;
        }

        $previous = $this->biomeColumn($terrain, $x, $z);
        if ($previous?->word() !== $biome->word()) {
            $this->biomes[$key] = $biome;
        }

        return $previous;
    }

    public function setBiome(
        ChunkTerrain $terrain,
        int $x,
        int $z,
        BiomeId $biome,
    ): ?BiomeId {
        $previous = $this->biomeColumn($terrain, $x, $z);
        if ($previous === null) {
            return null;
        }

        $this->setBiomeColumn($terrain, $x, $z, $previous->withId($biome));

        return $previous->id;
    }

    public function setBiomeColor(
        ChunkTerrain $terrain,
        int $x,
        int $z,
        int $color,
    ): ?int {
        $previous = $this->biomeColumn($terrain, $x, $z);
        if ($previous === null) {
            return null;
        }

        $this->setBiomeColumn($terrain, $x, $z, $previous->withColor($color));

        return $previous->color;
    }

    public function prepare(ChunkTerrain $terrain): PreparedTerrainPatch
    {
        $chunk = $terrain->data();
        $base = $terrain->revision()->value;

        $blocks = array_filter(
            $this->blocks,
            function (int $stateId, int $key) use ($chunk): bool {
                [$x, $y, $z] = self::decodeBlockKey($key);

                return $stateId !== $chunk->blockStateId($x, $y, $z);
            },
            ARRAY_FILTER_USE_BOTH,
        );
        $biomes = array_filter(
            $this->biomes,
            function (BiomeColumn $biome, int $key) use ($chunk): bool {
                [$x, $z] = self::decodeColumnKey($key);

                return $biome->word() !== $chunk->biomeColumn($x, $z)->word();
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
        return ChunkCoordinateKey::blockOrNull($x, $y, $z);
    }

    /** @internal @return array{int, int, int} */
    public static function decodeBlockKey(int $key): array
    {
        return ChunkCoordinateKey::decodeBlock($key);
    }

    private static function columnKey(int $x, int $z): ?int
    {
        return ChunkCoordinateKey::columnOrNull($x, $z);
    }

    /** @internal @return array{int, int} */
    public static function decodeColumnKey(int $key): array
    {
        return ChunkCoordinateKey::decodeColumn($key);
    }
}
