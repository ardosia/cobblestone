<?php

declare(strict_types=1);

namespace Cobblestone\World;

use Cobblestone\Native\World as NativeWorld;
use Cobblestone\Native\World\SnapshotDecoder;
use ValueError;

final class Chunk
{
    public const LIFECYCLE_GENERATED = 0x01;
    public const LIFECYCLE_POPULATED = 0x02;
    public const LIFECYCLE_LIGHT_POPULATED = 0x04;

    public function __construct(
        private readonly ChunkPos $position,
        private readonly NativeWorld $nativeStore,
        ?BiomeId $biome = null,
        bool $nativeResident = false,
    ) {
        if (!$nativeResident) {
            $this->nativeStore->ensureChunk($this->position, $biome ?? new BiomeId(1));
        }
    }

    public function position(): ChunkPos
    {
        return $this->position;
    }

    public function revision(): int
    {
        return $this->nativeStore->terrainRevision($this->position);
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
        return new LightRevision($this->nativeStore->lightRevision($this->position));
    }

    /** @internal Mutation commit primitive. */
    public function commitRevision(int $expected, int $next): void
    {
        $this->nativeStore->commitTerrainRevision($this->position, $expected, $next);
    }

    /** @internal Light-commit primitive. */
    public function commitLightRevision(int $expected, int $next): void
    {
        $this->nativeStore->commitLightRevision($this->position, $expected, $next);
    }

    public function blockStateId(int $x, int $y, int $z): int
    {
        self::assertBlockCoordinates($x, $y, $z);

        return $this->nativeStore->blockStateId($this->position, $x, $y, $z);
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

        return $this->nativeStore->setBlockStateId($this->position, $x, $y, $z, $stateId);
    }

    /** @internal Initialization or prepared-mutation commit primitive. */
    public function setBlock(int $x, int $y, int $z, BlockState $state): BlockState
    {
        return BlockState::fromId($this->setBlockStateId($x, $y, $z, $state->stateId()));
    }

    /**
     * Fills complete global chunk-local Y layers with one scalar state token.
     *
     * @internal Generator initialization primitive.
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

        $this->nativeStore->fillLayers($this->position, $startY, $count, $stateId);
    }

    public function skyLight(int $x, int $y, int $z): int
    {
        self::assertBlockCoordinates($x, $y, $z);

        return $this->nativeStore->skyLight($this->position, $x, $y, $z);
    }

    /** @internal Initialization or prepared-mutation commit primitive. */
    public function setSkyLight(int $x, int $y, int $z, int $level): int
    {
        self::assertBlockCoordinates($x, $y, $z);

        return $this->nativeStore->setSkyLight($this->position, $x, $y, $z, $level);
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

        $this->nativeStore->fillSkyLightFrom($this->position, $y, $level);
    }

    public function blockLight(int $x, int $y, int $z): int
    {
        self::assertBlockCoordinates($x, $y, $z);

        return $this->nativeStore->blockLight($this->position, $x, $y, $z);
    }

    /** @internal Initialization or prepared-mutation commit primitive. */
    public function setBlockLight(int $x, int $y, int $z, int $level): int
    {
        self::assertBlockCoordinates($x, $y, $z);

        return $this->nativeStore->setBlockLight($this->position, $x, $y, $z, $level);
    }

    public function biomeColumn(int $x, int $z): BiomeColumn
    {
        self::assertColumnCoordinates($x, $z);

        return BiomeColumn::fromWord($this->nativeStore->biomeWord($this->position, $x, $z));
    }

    public function biome(int $x, int $z): BiomeId
    {
        return $this->biomeColumn($x, $z)->id;
    }

    public function biomeColor(int $x, int $z): int
    {
        return $this->biomeColumn($x, $z)->color;
    }

    /** @internal Initialization or prepared-mutation commit primitive. */
    public function setBiomeColumn(int $x, int $z, BiomeColumn $biome): BiomeColumn
    {
        self::assertColumnCoordinates($x, $z);

        return BiomeColumn::fromWord(
            $this->nativeStore->setBiomeWord($this->position, $x, $z, $biome->word()),
        );
    }

    /** @internal Initialization or prepared-mutation commit primitive. */
    public function setBiome(int $x, int $z, BiomeId $biome): BiomeId
    {
        $previous = $this->biomeColumn($x, $z);
        $this->setBiomeColumn($x, $z, $previous->withId($biome));

        return $previous->id;
    }

    /** @internal Initialization or prepared-mutation commit primitive. */
    public function setBiomeColor(int $x, int $z, int $color): int
    {
        $previous = $this->biomeColumn($x, $z);
        $this->setBiomeColumn($x, $z, $previous->withColor($color));

        return $previous->color;
    }

    public function highestBlockAt(int $x, int $z): int
    {
        self::assertColumnCoordinates($x, $z);

        return $this->nativeStore->heightMap($this->position, $x, $z);
    }

    public function heightMap(int $x, int $z): int
    {
        self::assertColumnCoordinates($x, $z);

        return $this->nativeStore->heightMap($this->position, $x, $z);
    }

    public function recalculateHeightMap(): void
    {
        $this->nativeStore->recalculateHeightMap($this->position);
    }

    public function blockExtraData(int $x, int $y, int $z): int
    {
        self::assertBlockCoordinates($x, $y, $z);

        return $this->nativeStore->blockExtraData($this->position, $x, $y, $z);
    }

    /** @internal Initialization or prepared-mutation commit primitive. */
    public function setBlockExtraData(int $x, int $y, int $z, int $data): int
    {
        self::assertBlockCoordinates($x, $y, $z);
        if ($data < 0 || $data > 0xffff) {
            throw new ValueError('fixed-target block extra data must be in range 0..65535');
        }

        return $this->nativeStore->setBlockExtraData($this->position, $x, $y, $z, $data);
    }

    /** @return array<int, int> */
    public function extraData(): array
    {
        return $this->snapshot()->extraData;
    }

    /** Captures immutable semantic chunk state. */
    public function snapshot(): ChunkSnapshot
    {
        return SnapshotDecoder::decode(
            $this->position,
            $this->nativeStore->snapshotProjection($this->position),
        );
    }

    public function lightSnapshot(): LightSnapshot
    {
        return $this->snapshot()->light();
    }

    /**
     * @param array<int, int> $blocks
     * @param array<int, BiomeColumn> $biomes
     * @param array<int, int> $extraData
     * @param array<int, int> $skyLight
     * @param array<int, int> $blockLight
     *
     * @internal Prepared mutation batch primitive.
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
    ): void {
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

    public function lifecycleFlags(): int
    {
        return $this->nativeStore->lifecycleFlags($this->position);
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
        return $this->nativeStore->chunkDirty($this->position);
    }

    /** @internal Persistence completion primitive. */
    public function markPersisted(
        int $terrainRevision,
        int $lightRevision,
        int $lifecycleFlags,
    ): void {
        $this->nativeStore->markPersisted(
            $this->position,
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

    /** @internal */
    public function pinBacking(): void
    {
        $this->nativeStore->pinChunk($this->position);
    }

    /** @internal */
    public function unpinBacking(): void
    {
        $this->nativeStore->unpinChunk($this->position);
    }

    /** @internal */
    public function tryEvictBacking(int $_localPinCount): ChunkUnloadStatus
    {
        return match ($this->nativeStore->tryEvictChunk($this->position)) {
            0 => ChunkUnloadStatus::Missing,
            1 => ChunkUnloadStatus::Pinned,
            2 => ChunkUnloadStatus::Dirty,
            3 => ChunkUnloadStatus::Unloaded,
            default => throw new \UnexpectedValueException('invalid native chunk eviction status'),
        };
    }

    /** @internal */
    public function matchesNativeStore(NativeWorld $store): bool
    {
        return $store === $this->nativeStore;
    }

    private function setLifecycleFlags(int $flags): void
    {
        $this->nativeStore->setLifecycleFlags($this->position, $flags);
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

    private static function assertColumnCoordinates(int $x, int $z): void
    {
        if (!WorldBounds::containsLocal($x) || !WorldBounds::containsLocal($z)) {
            throw new ValueError('chunk column coordinates must be in range 0..15');
        }
    }
}
