<?php

declare(strict_types=1);

namespace Cobblestone\World\Internal;

use Cobblestone\Native\World as NativeWorld;
use Cobblestone\World\BiomeId;
use Cobblestone\World\ChunkSnapshot;
use Cobblestone\World\ChunkUnloadStatus;
use Cobblestone\World\LightSnapshot;

/** @internal Two real chunk-state implementations: native and PHP fallback. */
interface ChunkState
{
    public function revision(): int;

    public function lightRevision(): int;

    public function commitRevision(int $expected, int $next): void;

    public function commitLightRevision(int $expected, int $next): void;

    public function blockStateId(int $x, int $y, int $z): int;

    public function setBlockStateId(int $x, int $y, int $z, int $stateId): int;

    public function fillBlockLayers(int $startY, int $count, int $stateId): void;

    public function skyLight(int $x, int $y, int $z): int;

    public function setSkyLight(int $x, int $y, int $z, int $level): int;

    public function fillSkyLightFrom(int $y, int $level): void;

    public function blockLight(int $x, int $y, int $z): int;

    public function setBlockLight(int $x, int $y, int $z, int $level): int;

    public function biome(int $x, int $z): BiomeId;

    public function setBiome(int $x, int $z, BiomeId $biome): BiomeId;

    public function highestBlockAt(int $x, int $z): int;

    public function heightMap(int $x, int $z): int;

    public function recalculateHeightMap(): void;

    public function blockExtraData(int $x, int $y, int $z): int;

    public function setBlockExtraData(int $x, int $y, int $z, int $data): int;

    /** @return array<int, int> */
    public function extraData(): array;

    public function snapshot(): ChunkSnapshot;

    public function lightSnapshot(): LightSnapshot;

    /**
     * @param array<int, int> $blocks
     * @param array<int, BiomeId> $biomes
     * @param array<int, int> $extraData
     * @param array<int, int> $skyLight
     * @param array<int, int> $blockLight
     */
    public function applyPatch(
        int $expectedTerrainRevision,
        int $nextTerrainRevision,
        int $expectedLightRevision,
        int $nextLightRevision,
        array $blocks,
        array $biomes,
        array $extraData,
        array $skyLight,
        array $blockLight,
    ): void;

    public function lifecycleFlags(): int;

    public function setLifecycleFlags(int $flags): void;

    public function isDirty(): bool;

    public function markPersisted(
        int $terrainRevision,
        int $lightRevision,
        int $lifecycleFlags,
    ): void;

    public function pin(): void;

    public function unpin(): void;

    public function tryEvict(int $localPinCount): ChunkUnloadStatus;

    public function matchesNativeStore(?NativeWorld $store): bool;

    public function prefersSnapshotReads(): bool;
}
