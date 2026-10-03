<?php

declare(strict_types=1);

namespace Cobblestone\World;

use Cobblestone\Native\World as NativeWorld;
use Cobblestone\World\Internal\ChunkState;
use Cobblestone\World\Internal\FallbackChunkState;
use Cobblestone\World\Internal\NativeChunkState;
use ValueError;

final class Chunk
{
    public const LIFECYCLE_GENERATED = 0x01;
    public const LIFECYCLE_POPULATED = 0x02;
    public const LIFECYCLE_LIGHT_POPULATED = 0x04;

    private readonly ChunkState $state;

    public function __construct(
        private readonly ChunkPos $position,
        ?BiomeId $biome = null,
        ?NativeWorld $nativeStore = null,
        bool $nativeResident = false,
    ) {
        $biome ??= new BiomeId(1);
        $this->state = $nativeStore === null
            ? new FallbackChunkState($this->position, $biome)
            : new NativeChunkState($nativeStore, $this->position, $biome, $nativeResident);
    }

    public function position(): ChunkPos
    {
        return $this->position;
    }

    public function revision(): int
    {
        return $this->state->revision();
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
        return new LightRevision($this->state->lightRevision());
    }

    /** @internal Mutation commit primitive. */
    public function commitRevision(int $expected, int $next): void
    {
        $this->state->commitRevision($expected, $next);
    }

    /** @internal Light-commit primitive. */
    public function commitLightRevision(int $expected, int $next): void
    {
        $this->state->commitLightRevision($expected, $next);
    }

    public function blockStateId(int $x, int $y, int $z): int
    {
        self::assertBlockCoordinates($x, $y, $z);

        return $this->state->blockStateId($x, $y, $z);
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

        return $this->state->setBlockStateId($x, $y, $z, $stateId);
    }

    /** @internal Initialization or prepared-mutation commit primitive. */
    public function setBlock(int $x, int $y, int $z, BlockState $state): BlockState
    {
        return BlockState::fromId($this->setBlockStateId($x, $y, $z, $state->fullId()));
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

        $this->state->fillBlockLayers($startY, $count, $stateId);
    }

    public function skyLight(int $x, int $y, int $z): int
    {
        self::assertBlockCoordinates($x, $y, $z);

        return $this->state->skyLight($x, $y, $z);
    }

    /** @internal Initialization or prepared-mutation commit primitive. */
    public function setSkyLight(int $x, int $y, int $z, int $level): int
    {
        self::assertBlockCoordinates($x, $y, $z);

        return $this->state->setSkyLight($x, $y, $z, $level);
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

        $this->state->fillSkyLightFrom($y, $level);
    }

    public function blockLight(int $x, int $y, int $z): int
    {
        self::assertBlockCoordinates($x, $y, $z);

        return $this->state->blockLight($x, $y, $z);
    }

    /** @internal Initialization or prepared-mutation commit primitive. */
    public function setBlockLight(int $x, int $y, int $z, int $level): int
    {
        self::assertBlockCoordinates($x, $y, $z);

        return $this->state->setBlockLight($x, $y, $z, $level);
    }

    public function biomeColumn(int $x, int $z): BiomeColumn
    {
        self::assertColumnCoordinates($x, $z);

        return $this->state->biomeColumn($x, $z);
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

        return $this->state->setBiomeColumn($x, $z, $biome);
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

        return $this->state->highestBlockAt($x, $z);
    }

    public function heightMap(int $x, int $z): int
    {
        self::assertColumnCoordinates($x, $z);

        return $this->state->heightMap($x, $z);
    }

    public function recalculateHeightMap(): void
    {
        $this->state->recalculateHeightMap();
    }

    public function blockExtraData(int $x, int $y, int $z): int
    {
        self::assertBlockCoordinates($x, $y, $z);

        return $this->state->blockExtraData($x, $y, $z);
    }

    /** @internal Initialization or prepared-mutation commit primitive. */
    public function setBlockExtraData(int $x, int $y, int $z, int $data): int
    {
        self::assertBlockCoordinates($x, $y, $z);
        if ($data < 0 || $data > 0xffff) {
            throw new ValueError('fixed-target block extra data must be in range 0..65535');
        }

        return $this->state->setBlockExtraData($x, $y, $z, $data);
    }

    /** @return array<int, int> */
    public function extraData(): array
    {
        return $this->state->extraData();
    }

    /** Captures immutable semantic chunk state. */
    public function snapshot(): ChunkSnapshot
    {
        return $this->state->snapshot();
    }

    public function lightSnapshot(): LightSnapshot
    {
        return $this->state->lightSnapshot();
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
        $this->state->applyPatch(
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
        return $this->state->lifecycleFlags();
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
        return $this->state->isDirty();
    }

    /** @internal Persistence completion primitive. */
    public function markPersisted(
        int $terrainRevision,
        int $lightRevision,
        int $lifecycleFlags,
    ): void {
        $this->state->markPersisted(
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
        $this->state->pin();
    }

    /** @internal */
    public function unpinBacking(): void
    {
        $this->state->unpin();
    }

    /** @internal */
    public function tryEvictBacking(int $localPinCount): ChunkUnloadStatus
    {
        return $this->state->tryEvict($localPinCount);
    }

    /** @internal */
    public function matchesNativeStore(?NativeWorld $store): bool
    {
        return $this->state->matchesNativeStore($store);
    }

    /** @internal Mutation prepare optimization hint. */
    public function prefersSnapshotReads(): bool
    {
        return $this->state->prefersSnapshotReads();
    }

    private function setLifecycleFlags(int $flags): void
    {
        $this->state->setLifecycleFlags($flags);
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
