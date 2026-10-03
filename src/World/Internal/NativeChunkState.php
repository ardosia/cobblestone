<?php

declare(strict_types=1);

namespace Cobblestone\World\Internal;

use Cobblestone\Native\World as NativeWorld;
use Cobblestone\Native\World\SnapshotDecoder;
use Cobblestone\World\BiomeId;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\ChunkSnapshot;
use Cobblestone\World\ChunkUnloadStatus;
use Cobblestone\World\LightSnapshot;

/** @internal Native WorldStore-backed state for exactly one chunk coordinate. */
final readonly class NativeChunkState implements ChunkState
{
    public function __construct(
        private NativeWorld $store,
        private ChunkPos $position,
        BiomeId $biome,
        bool $resident,
    ) {
        if (!$resident) {
            $this->store->ensureChunk($this->position, $biome);
        }
    }

    public function revision(): int
    {
        return $this->store->terrainRevision($this->position);
    }

    public function lightRevision(): int
    {
        return $this->store->lightRevision($this->position);
    }

    public function commitRevision(int $expected, int $next): void
    {
        $this->store->commitTerrainRevision($this->position, $expected, $next);
    }

    public function commitLightRevision(int $expected, int $next): void
    {
        $this->store->commitLightRevision($this->position, $expected, $next);
    }

    public function blockStateId(int $x, int $y, int $z): int
    {
        return $this->store->blockStateId($this->position, $x, $y, $z);
    }

    public function setBlockStateId(int $x, int $y, int $z, int $stateId): int
    {
        return $this->store->setBlockStateId($this->position, $x, $y, $z, $stateId);
    }

    public function fillBlockLayers(int $startY, int $count, int $stateId): void
    {
        $this->store->fillLayers($this->position, $startY, $count, $stateId);
    }

    public function skyLight(int $x, int $y, int $z): int
    {
        return $this->store->skyLight($this->position, $x, $y, $z);
    }

    public function setSkyLight(int $x, int $y, int $z, int $level): int
    {
        return $this->store->setSkyLight($this->position, $x, $y, $z, $level);
    }

    public function fillSkyLightFrom(int $y, int $level): void
    {
        $this->store->fillSkyLightFrom($this->position, $y, $level);
    }

    public function blockLight(int $x, int $y, int $z): int
    {
        return $this->store->blockLight($this->position, $x, $y, $z);
    }

    public function setBlockLight(int $x, int $y, int $z, int $level): int
    {
        return $this->store->setBlockLight($this->position, $x, $y, $z, $level);
    }

    public function biome(int $x, int $z): BiomeId
    {
        return new BiomeId($this->store->biome($this->position, $x, $z));
    }

    public function setBiome(int $x, int $z, BiomeId $biome): BiomeId
    {
        return new BiomeId(
            $this->store->setBiome($this->position, $x, $z, $biome->value),
        );
    }

    public function highestBlockAt(int $x, int $z): int
    {
        return $this->store->heightMap($this->position, $x, $z);
    }

    public function heightMap(int $x, int $z): int
    {
        return $this->store->heightMap($this->position, $x, $z);
    }

    public function recalculateHeightMap(): void
    {
        $this->store->recalculateHeightMap($this->position);
    }

    public function blockExtraData(int $x, int $y, int $z): int
    {
        return $this->store->blockExtraData($this->position, $x, $y, $z);
    }

    public function setBlockExtraData(int $x, int $y, int $z, int $data): int
    {
        return $this->store->setBlockExtraData($this->position, $x, $y, $z, $data);
    }

    public function extraData(): array
    {
        return $this->snapshot()->extraData;
    }

    public function snapshot(): ChunkSnapshot
    {
        return SnapshotDecoder::decode(
            $this->position,
            $this->store->snapshotProjection($this->position),
        );
    }

    public function lightSnapshot(): LightSnapshot
    {
        return $this->snapshot()->light();
    }

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
    ): void {
        $this->store->applyPatch(
            $this->position,
            $expectedTerrainRevision,
            $nextTerrainRevision,
            $expectedLightRevision,
            $nextLightRevision,
            $blocks,
            $biomes,
            $extraData,
            $skyLight,
            $blockLight,
        );
    }

    public function lifecycleFlags(): int
    {
        return $this->store->lifecycleFlags($this->position);
    }

    public function setLifecycleFlags(int $flags): void
    {
        $this->store->setLifecycleFlags($this->position, $flags);
    }

    public function isDirty(): bool
    {
        return $this->store->chunkDirty($this->position);
    }

    public function markPersisted(
        int $terrainRevision,
        int $lightRevision,
        int $lifecycleFlags,
    ): void {
        $this->store->markPersisted(
            $this->position,
            $terrainRevision,
            $lightRevision,
            $lifecycleFlags,
        );
    }

    public function pin(): void
    {
        $this->store->pinChunk($this->position);
    }

    public function unpin(): void
    {
        $this->store->unpinChunk($this->position);
    }

    public function tryEvict(int $_localPinCount): ChunkUnloadStatus
    {
        return match ($this->store->tryEvictChunk($this->position)) {
            0 => ChunkUnloadStatus::Missing,
            1 => ChunkUnloadStatus::Pinned,
            2 => ChunkUnloadStatus::Dirty,
            3 => ChunkUnloadStatus::Unloaded,
            default => throw new \UnexpectedValueException(
                'invalid native chunk eviction status',
            ),
        };
    }

    public function matchesNativeStore(?NativeWorld $store): bool
    {
        return $store === $this->store;
    }

    public function prefersSnapshotReads(): bool
    {
        return true;
    }
}
