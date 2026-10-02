<?php

declare(strict_types=1);

namespace Cobblestone\World;

use ValueError;

final class Chunk
{
    public const LIFECYCLE_GENERATED = 0x01;
    public const LIFECYCLE_POPULATED = 0x02;
    public const LIFECYCLE_LIGHT_POPULATED = 0x04;

    private readonly ?ChunkFallbackState $fallbackState;

    public function __construct(
        private readonly ChunkPos $position,
        ?BiomeId $biome = null,
        private readonly ?NativeWorldStore $nativeStore = null,
        bool $nativeResident = false,
    ) {
        $biome ??= new BiomeId(1);
        $this->fallbackState = $this->nativeStore === null ? new ChunkFallbackState($biome) : null;

        if ($this->nativeStore !== null) {
            if (!$nativeResident) {
                $this->nativeStore->ensureChunk($this->position, $biome);
            }
            return;
        }

    }

    public function position(): ChunkPos
    {
        return $this->position;
    }

    public function revision(): int
    {
        return $this->nativeStore?->terrainRevision($this->position) ?? $this->fallbackState()->revision();
    }

    public function terrainRevision(): ChunkRevision
    {
        return new ChunkRevision($this->revision());
    }

    public function terrain(): ChunkTerrain
    {
        return new ChunkTerrain($this);
    }

    public function light(): ChunkLight
    {
        return new ChunkLight($this);
    }

    public function lightRevision(): LightRevision
    {
        return new LightRevision(
            $this->nativeStore?->lightRevision($this->position) ?? $this->fallbackState()->lightRevision(),
        );
    }

    /** @internal Mutation commit primitive. */
    public function commitRevision(int $expected, int $next): void
    {
        if ($this->nativeStore !== null) {
            $this->nativeStore->commitTerrainRevision($this->position, $expected, $next);
            return;
        }

        $this->fallbackState()->commitRevision($expected, $next);
    }

    /** @internal Light-commit primitive. */
    public function commitLightRevision(int $expected, int $next): void
    {
        if ($this->nativeStore !== null) {
            $this->nativeStore->commitLightRevision($this->position, $expected, $next);
            return;
        }

        $this->fallbackState()->commitLightRevision($expected, $next);
    }

    public function blockStateId(int $x, int $y, int $z): int
    {
        self::assertBlockCoordinates($x, $y, $z);
        if ($this->nativeStore !== null) {
            return $this->nativeStore->blockStateId($this->position, $x, $y, $z);
        }

        return $this->fallbackState()->blockStateId($x, $y, $z);
    }

    public function block(int $x, int $y, int $z): BlockState
    {
        return BlockState::fromId($this->blockStateId($x, $y, $z));
    }

    /** @internal Initialization or prepared-mutation commit primitive. */
    public function setBlockStateId(int $x, int $y, int $z, int $stateId): int
    {
        BlockStateId::assert($stateId);
        self::assertBlockCoordinates($x, $y, $z);
        if ($this->nativeStore !== null) {
            return $this->nativeStore->setBlockStateId($this->position, $x, $y, $z, $stateId);
        }

        return $this->fallbackState()->setBlockStateId($x, $y, $z, $stateId);
    }

    /** @internal Initialization or prepared-mutation commit primitive. */
    public function setBlock(int $x, int $y, int $z, BlockState $state): BlockState
    {
        return BlockState::fromId($this->setBlockStateId($x, $y, $z, $state->fullId()));
    }

    /**
     * Fills complete global chunk-local Y layers with one scalar state token.
     *
     * @internal Generator/native-fallback initialization primitive.
     */
    public function fillBlockLayers(int $startY, int $count, int $stateId): void
    {
        BlockStateId::assert($stateId);
        if ($startY < WorldBounds::MIN_Y || $startY > WorldBounds::WORLD_HEIGHT) {
            throw new ValueError('chunk layer start must be in range 0..128');
        }
        if ($count < 0 || $startY + $count > WorldBounds::WORLD_HEIGHT) {
            throw new ValueError('chunk layer range must stay inside 0..128');
        }
        if ($count === 0) {
            return;
        }
        if ($this->nativeStore !== null) {
            $this->nativeStore->fillLayers($this->position, $startY, $count, $stateId);
            return;
        }

        $this->fallbackState()->fillBlockLayers($startY, $count, $stateId);
    }

    public function skyLight(int $x, int $y, int $z): int
    {
        self::assertBlockCoordinates($x, $y, $z);
        if ($this->nativeStore !== null) {
            return $this->nativeStore->skyLight($this->position, $x, $y, $z);
        }

        return $this->fallbackState()->skyLight($x, $y, $z);
    }

    /** @internal Initialization or prepared-mutation commit primitive. */
    public function setSkyLight(int $x, int $y, int $z, int $level): int
    {
        self::assertBlockCoordinates($x, $y, $z);
        if ($this->nativeStore !== null) {
            return $this->nativeStore->setSkyLight($this->position, $x, $y, $z, $level);
        }

        return $this->fallbackState()->setSkyLight($x, $y, $z, $level);
    }

    /**
     * Fills semantic sky light from a global chunk-local Y through the world top.
     *
     * @internal Generator initialization primitive.
     */
    public function fillSkyLightFrom(int $y, int $level): void
    {
        if ($y < WorldBounds::MIN_Y || $y > WorldBounds::WORLD_HEIGHT) {
            throw new ValueError('chunk sky-light fill y must be in range 0..128');
        }
        if ($level < 0 || $level > 0x0f) {
            throw new ValueError('fixed-target light level must be in range 0..15');
        }
        if ($this->nativeStore !== null) {
            $this->nativeStore->fillSkyLightFrom($this->position, $y, $level);
            return;
        }

        $this->fallbackState()->fillSkyLightFrom($y, $level);
    }

    public function blockLight(int $x, int $y, int $z): int
    {
        self::assertBlockCoordinates($x, $y, $z);
        if ($this->nativeStore !== null) {
            return $this->nativeStore->blockLight($this->position, $x, $y, $z);
        }

        return $this->fallbackState()->blockLight($x, $y, $z);
    }

    /** @internal Initialization or prepared-mutation commit primitive. */
    public function setBlockLight(int $x, int $y, int $z, int $level): int
    {
        self::assertBlockCoordinates($x, $y, $z);
        if ($this->nativeStore !== null) {
            return $this->nativeStore->setBlockLight($this->position, $x, $y, $z, $level);
        }

        return $this->fallbackState()->setBlockLight($x, $y, $z, $level);
    }

    public function biome(int $x, int $z): BiomeId
    {
        $index = self::columnIndex($x, $z);
        if ($this->nativeStore !== null) {
            return new BiomeId($this->nativeStore->biome($this->position, $x, $z));
        }

        return $this->fallbackState()->biome($index);
    }

    /** @internal Initialization or prepared-mutation commit primitive. */
    public function setBiome(int $x, int $z, BiomeId $biome): BiomeId
    {
        $index = self::columnIndex($x, $z);
        if ($this->nativeStore !== null) {
            return new BiomeId(
                $this->nativeStore->setBiome($this->position, $x, $z, $biome->value),
            );
        }

        return $this->fallbackState()->setBiome($index, $biome);
    }

    public function highestBlockAt(int $x, int $z): int
    {
        self::columnIndex($x, $z);
        if ($this->nativeStore !== null) {
            return $this->nativeStore->heightMap($this->position, $x, $z);
        }

        return $this->fallbackState()->highestBlockAt($x, $z);
    }

    public function heightMap(int $x, int $z): int
    {
        $index = self::columnIndex($x, $z);
        if ($this->nativeStore !== null) {
            return $this->nativeStore->heightMap($this->position, $x, $z);
        }

        return $this->fallbackState()->heightAt($index);
    }

    public function recalculateHeightMap(): void
    {
        if ($this->nativeStore !== null) {
            $this->nativeStore->recalculateHeightMap($this->position);
            return;
        }

        $this->fallbackState()->recalculateHeightMap();
    }

    public function blockExtraData(int $x, int $y, int $z): int
    {
        self::assertBlockCoordinates($x, $y, $z);
        if ($this->nativeStore !== null) {
            return $this->nativeStore->blockExtraData($this->position, $x, $y, $z);
        }

        return $this->fallbackState()->blockExtraData(self::extraDataKey($x, $y, $z));
    }

    /** @internal Initialization or prepared-mutation commit primitive. */
    public function setBlockExtraData(int $x, int $y, int $z, int $data): int
    {
        self::assertBlockCoordinates($x, $y, $z);
        if ($data < 0 || $data > 0xffff) {
            throw new ValueError('fixed-target block extra data must be in range 0..65535');
        }
        if ($this->nativeStore !== null) {
            return $this->nativeStore->setBlockExtraData($this->position, $x, $y, $z, $data);
        }

        return $this->fallbackState()->setBlockExtraData(
            self::extraDataKey($x, $y, $z),
            $data,
        );
    }

    /** @return array<int, int> */
    public function extraData(): array
    {
        return $this->nativeStore !== null
            ? $this->nativeSnapshot()->extraData
            : $this->fallbackState()->extraData();
    }

    /**
     * Captures immutable semantic chunk state without exposing mutable section objects.
     */
    public function snapshot(): ChunkSnapshot
    {
        return $this->nativeStore !== null
            ? $this->nativeSnapshot()
            : $this->fallbackState()->snapshot($this->position);
    }

    public function lightSnapshot(): LightSnapshot
    {
        return $this->nativeStore !== null
            ? $this->nativeSnapshot()->light()
            : $this->fallbackState()->lightSnapshot();
    }


    /**
     * @param array<int, int> $blocks
     * @param array<int, BiomeId> $biomes
     * @param array<int, int> $extraData
     * @param array<int, int> $skyLight
     * @param array<int, int> $blockLight
     *
     * @internal Prepared mutation batch primitive.
     */
    public function applyNativePatch(
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
        if ($this->nativeStore === null) {
            throw new \LogicException('native patch requested for a PHP-backed chunk');
        }

        $this->nativeStore->applyPatch(
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

    /** @internal */
    public function nativeStore(): ?NativeWorldStore
    {
        return $this->nativeStore;
    }

    public function lifecycleFlags(): int
    {
        if ($this->nativeStore !== null) {
            return $this->nativeStore->lifecycleFlags($this->position);
        }

        return $this->fallbackState()->lifecycleFlags();
    }

    public function isGenerated(): bool
    {
        return ($this->lifecycleFlags() & self::LIFECYCLE_GENERATED) !== 0;
    }

    public function markGenerated(bool $generated = true): void
    {
        $flags = $this->lifecycleFlags();
        $this->setLifecycleFlags(
            $generated
                ? $flags | self::LIFECYCLE_GENERATED
                : $flags & ~self::LIFECYCLE_GENERATED,
        );
    }

    public function isPopulated(): bool
    {
        return ($this->lifecycleFlags() & self::LIFECYCLE_POPULATED) !== 0;
    }

    public function markPopulated(bool $populated = true): void
    {
        $flags = $this->lifecycleFlags();
        $this->setLifecycleFlags(
            $populated
                ? $flags | self::LIFECYCLE_POPULATED
                : $flags & ~self::LIFECYCLE_POPULATED,
        );
    }

    public function isLightPopulated(): bool
    {
        return ($this->lifecycleFlags() & self::LIFECYCLE_LIGHT_POPULATED) !== 0;
    }

    public function markLightPopulated(bool $lightPopulated = true): void
    {
        $flags = $this->lifecycleFlags();
        $this->setLifecycleFlags(
            $lightPopulated
                ? $flags | self::LIFECYCLE_LIGHT_POPULATED
                : $flags & ~self::LIFECYCLE_LIGHT_POPULATED,
        );
    }

    /** @internal Persistence/lifecycle primitive. */
    public function isDirty(): bool
    {
        if ($this->nativeStore !== null) {
            return $this->nativeStore->chunkDirty($this->position);
        }

        return $this->fallbackState()->isDirty();
    }

    /** @internal Persistence completion primitive. */
    public function markPersisted(
        int $terrainRevision,
        int $lightRevision,
        int $lifecycleFlags,
    ): void {
        if ($this->nativeStore !== null) {
            $this->nativeStore->markPersisted(
                $this->position,
                $terrainRevision,
                $lightRevision,
                $lifecycleFlags,
            );
            return;
        }

        $this->fallbackState()->markPersisted(
            $terrainRevision,
            $lightRevision,
            $lifecycleFlags,
        );
    }

    /** @internal Test/bootstrap helper until asynchronous persistence owns completion. */
    public function markCurrentStatePersisted(): void
    {
        $this->markPersisted(
            $this->revision(),
            $this->lightRevision()->value,
            $this->lifecycleFlags(),
        );
    }

    private function setLifecycleFlags(int $flags): void
    {
        if ($this->nativeStore !== null) {
            $this->nativeStore->setLifecycleFlags($this->position, $flags);
            return;
        }

        $this->fallbackState()->setLifecycleFlags($flags);
    }

    private function fallbackState(): ChunkFallbackState
    {
        return $this->fallbackState
            ?? throw new \LogicException('PHP fallback state requested for a native-backed chunk');
    }

    private function nativeSnapshot(): ChunkSnapshot
    {
        $projection = $this->nativeStore?->snapshotProjection($this->position)
            ?? throw new \LogicException('native chunk snapshot requested without a native store');

        return NativeChunkSnapshotDecoder::decode($this->position, $projection);
    }

    private static function assertBlockCoordinates(int $x, int $y, int $z): void
    {
        if (
            !WorldBounds::containsLocal($x)
            || !WorldBounds::containsLocal($z)
            || !WorldBounds::containsY($y)
        ) {
            throw new ValueError('chunk block coordinates must be x/z 0..15 and y 0..127');
        }
    }

    private static function columnIndex(int $x, int $z): int
    {
        if (!WorldBounds::containsLocal($x) || !WorldBounds::containsLocal($z)) {
            throw new ValueError('chunk column coordinates must be in range 0..15');
        }

        return ($z << 4) | $x;
    }

    private static function extraDataKey(int $x, int $y, int $z): int
    {
        return ($z << 12) | ($x << 8) | $y;
    }
}
