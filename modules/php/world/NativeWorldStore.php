<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Internal owner-runtime bridge to the Rust world store.
 *
 * Plugin-facing APIs stay on World/Chunk; this class keeps the native handle and FFI names out of
 * normal gameplay code.
 *
 * @internal
 */
final class NativeWorldStore
{
    private ?int $handle;

    private function __construct(int $handle)
    {
        $this->handle = $handle;
    }

    public static function available(): bool
    {
        if (!extension_loaded('cobblestone_core_php')) {
            return false;
        }

        foreach ([
            'cobblestone_world_create',
            'cobblestone_world_destroy',
            'cobblestone_world_ensure_chunk',
            'cobblestone_world_terrain_revision',
            'cobblestone_world_light_revision',
            'cobblestone_world_commit_terrain_revision',
            'cobblestone_world_commit_light_revision',
            'cobblestone_world_block_state',
            'cobblestone_world_set_block_state',
            'cobblestone_world_fill_layers',
            'cobblestone_world_biome',
            'cobblestone_world_set_biome',
            'cobblestone_world_fill_biome',
            'cobblestone_world_sky_light',
            'cobblestone_world_set_sky_light',
            'cobblestone_world_fill_sky_light_from',
            'cobblestone_world_block_light',
            'cobblestone_world_set_block_light',
            'cobblestone_world_height_map',
            'cobblestone_world_recalculate_height_map',
            'cobblestone_world_block_extra_data',
            'cobblestone_world_set_block_extra_data',
            'cobblestone_world_snapshot',
        ] as $function) {
            if (!\function_exists($function)) {
                return false;
            }
        }

        return true;
    }

    public static function create(): self
    {
        if (!self::available()) {
            throw new \RuntimeException('cobblestone_core_php world store is unavailable or stale');
        }

        return new self(cobblestone_world_create());
    }

    public function handle(): int
    {
        return $this->requireHandle();
    }

    public function ensureChunk(ChunkPos $position, BiomeId $biome): bool
    {
        return cobblestone_world_ensure_chunk(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $biome->value,
        );
    }

    public function terrainRevision(ChunkPos $position): int
    {
        return cobblestone_world_terrain_revision(
            $this->requireHandle(),
            $position->x,
            $position->z,
        );
    }

    public function lightRevision(ChunkPos $position): int
    {
        return cobblestone_world_light_revision(
            $this->requireHandle(),
            $position->x,
            $position->z,
        );
    }

    public function commitTerrainRevision(ChunkPos $position, int $expected, int $next): void
    {
        cobblestone_world_commit_terrain_revision(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $expected,
            $next,
        );
    }

    public function commitLightRevision(ChunkPos $position, int $expected, int $next): void
    {
        cobblestone_world_commit_light_revision(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $expected,
            $next,
        );
    }

    public function blockStateId(ChunkPos $position, int $x, int $y, int $z): int
    {
        return cobblestone_world_block_state(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $x,
            $y,
            $z,
        );
    }

    public function setBlockStateId(ChunkPos $position, int $x, int $y, int $z, int $stateId): int
    {
        return cobblestone_world_set_block_state(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $x,
            $y,
            $z,
            $stateId,
        );
    }

    public function fillLayers(ChunkPos $position, int $startY, int $count, int $stateId): void
    {
        cobblestone_world_fill_layers(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $startY,
            $count,
            $stateId,
        );
    }

    public function biome(ChunkPos $position, int $x, int $z): int
    {
        return cobblestone_world_biome(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $x,
            $z,
        );
    }

    public function setBiome(ChunkPos $position, int $x, int $z, int $biome): int
    {
        return cobblestone_world_set_biome(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $x,
            $z,
            $biome,
        );
    }

    public function fillBiome(ChunkPos $position, int $biome): void
    {
        cobblestone_world_fill_biome(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $biome,
        );
    }

    public function skyLight(ChunkPos $position, int $x, int $y, int $z): int
    {
        return cobblestone_world_sky_light(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $x,
            $y,
            $z,
        );
    }

    public function setSkyLight(ChunkPos $position, int $x, int $y, int $z, int $level): int
    {
        return cobblestone_world_set_sky_light(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $x,
            $y,
            $z,
            $level,
        );
    }

    public function fillSkyLightFrom(ChunkPos $position, int $y, int $level): void
    {
        cobblestone_world_fill_sky_light_from(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $y,
            $level,
        );
    }

    public function blockLight(ChunkPos $position, int $x, int $y, int $z): int
    {
        return cobblestone_world_block_light(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $x,
            $y,
            $z,
        );
    }

    public function setBlockLight(ChunkPos $position, int $x, int $y, int $z, int $level): int
    {
        return cobblestone_world_set_block_light(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $x,
            $y,
            $z,
            $level,
        );
    }

    public function heightMap(ChunkPos $position, int $x, int $z): int
    {
        return cobblestone_world_height_map(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $x,
            $z,
        );
    }

    public function recalculateHeightMap(ChunkPos $position): void
    {
        cobblestone_world_recalculate_height_map(
            $this->requireHandle(),
            $position->x,
            $position->z,
        );
    }

    public function blockExtraData(ChunkPos $position, int $x, int $y, int $z): int
    {
        return cobblestone_world_block_extra_data(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $x,
            $y,
            $z,
        );
    }

    public function setBlockExtraData(
        ChunkPos $position,
        int $x,
        int $y,
        int $z,
        int $data,
    ): int {
        return cobblestone_world_set_block_extra_data(
            $this->requireHandle(),
            $position->x,
            $position->z,
            $x,
            $y,
            $z,
            $data,
        );
    }


    /**
     * Materializes one immutable chunk projection for compatibility/debug consumers.
     *
     * Layout: blockIds, blockData, skyLight, blockLight, biomes, heightMap, extraCount(u32le),
     * then extraData key/value u16le pairs.
     */
    public function snapshotProjection(ChunkPos $position): string
    {
        return cobblestone_world_snapshot(
            $this->requireHandle(),
            $position->x,
            $position->z,
        );
    }

    public function destroy(): void
    {
        if ($this->handle === null) {
            return;
        }

        $handle = $this->handle;
        $this->handle = null;
        cobblestone_world_destroy($handle);
    }

    public function __destruct()
    {
        if ($this->handle !== null && \function_exists('cobblestone_world_destroy')) {
            try {
                cobblestone_world_destroy($this->handle);
            } catch (\Throwable) {
                // Extension/module shutdown owns the final cleanup fallback.
            }
            $this->handle = null;
        }
    }

    private function requireHandle(): int
    {
        return $this->handle
            ?? throw new \LogicException('native world store has already been destroyed');
    }
}
