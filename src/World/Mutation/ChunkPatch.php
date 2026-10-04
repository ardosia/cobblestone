<?php

declare(strict_types=1);

namespace Cobblestone\World\Mutation;

use Cobblestone\World\BiomeColumn;
use Cobblestone\World\BiomeId;
use Cobblestone\World\BlockState;
use Cobblestone\World\BlockStateId;
use Cobblestone\World\Chunk;
use Cobblestone\World\ChunkCoordinateKey;
use Cobblestone\World\ChunkSnapshot;
use ValueError;

/** @internal */
final class ChunkPatch
{
    private const SNAPSHOT_PREPARE_THRESHOLD = 768;

    private readonly int $baseRevision;
    private readonly int $baseLightRevision;
    private ?ChunkSnapshot $snapshot = null;

    /** @var array<int, int> scalar BlockStateId tokens */
    private array $blocks = [];

    /** @var array<int, BiomeColumn> */
    private array $biomes = [];

    /** @var array<int, int> */
    private array $extraData = [];

    /** @var array<int, int> */
    private array $skyLight = [];

    /** @var array<int, int> */
    private array $blockLight = [];

    public function __construct(
        private readonly Chunk $chunk,
    ) {
        $this->baseRevision = $chunk->revision();
        $this->baseLightRevision = $chunk->lightRevision()->value;
    }

    public function chunk(): Chunk
    {
        return $this->chunk;
    }

    public function baseRevision(): int
    {
        return $this->baseRevision;
    }

    public function blockStateId(int $x, int $y, int $z): int
    {
        $key = self::blockKey($x, $y, $z);

        return $this->blocks[$key] ?? $this->blockStateIdAtKey($key);
    }

    public function block(int $x, int $y, int $z): BlockState
    {
        return BlockState::fromId($this->blockStateId($x, $y, $z));
    }

    public function setBlockStateId(int $x, int $y, int $z, int $stateId): int
    {
        BlockStateId::assert($stateId);
        $previous = $this->blockStateId($x, $y, $z);
        $this->blocks[self::blockKey($x, $y, $z)] = $stateId;

        return $previous;
    }

    public function setBlock(int $x, int $y, int $z, BlockState $state): BlockState
    {
        return BlockState::fromId(
            $this->setBlockStateId($x, $y, $z, $state->stateId()),
        );
    }

    public function biomeColumn(int $x, int $z): BiomeColumn
    {
        $key = self::columnKey($x, $z);

        return $this->biomes[$key] ?? $this->biomeColumnAtKey($key);
    }

    public function biome(int $x, int $z): BiomeId
    {
        return $this->biomeColumn($x, $z)->id;
    }

    public function biomeColor(int $x, int $z): int
    {
        return $this->biomeColumn($x, $z)->color;
    }

    public function setBiomeColumn(int $x, int $z, BiomeColumn $biome): BiomeColumn
    {
        $previous = $this->biomeColumn($x, $z);
        $this->biomes[self::columnKey($x, $z)] = $biome;

        return $previous;
    }

    public function setBiome(int $x, int $z, BiomeId $biome): BiomeId
    {
        $previous = $this->biomeColumn($x, $z);
        $this->setBiomeColumn($x, $z, $previous->withId($biome));

        return $previous->id;
    }

    public function setBiomeColor(int $x, int $z, int $color): int
    {
        $previous = $this->biomeColumn($x, $z);
        $this->setBiomeColumn($x, $z, $previous->withColor($color));

        return $previous->color;
    }

    public function blockExtraData(int $x, int $y, int $z): int
    {
        $key = self::blockKey($x, $y, $z);

        return $this->extraData[$key] ?? $this->extraDataAtKey($key);
    }

    public function setBlockExtraData(int $x, int $y, int $z, int $data): int
    {
        if ($data < 0 || $data > 0xffff) {
            throw new ValueError('fixed-target block extra data must be in range 0..65535');
        }

        $previous = $this->blockExtraData($x, $y, $z);
        $this->extraData[self::blockKey($x, $y, $z)] = $data;

        return $previous;
    }

    public function skyLight(int $x, int $y, int $z): int
    {
        $key = self::blockKey($x, $y, $z);

        return $this->skyLight[$key] ?? $this->skyLightAtKey($key);
    }

    public function setSkyLight(int $x, int $y, int $z, int $level): int
    {
        self::assertLight($level);
        $previous = $this->skyLight($x, $y, $z);
        $this->skyLight[self::blockKey($x, $y, $z)] = $level;

        return $previous;
    }

    public function blockLight(int $x, int $y, int $z): int
    {
        $key = self::blockKey($x, $y, $z);

        return $this->blockLight[$key] ?? $this->blockLightAtKey($key);
    }

    public function setBlockLight(int $x, int $y, int $z, int $level): int
    {
        self::assertLight($level);
        $previous = $this->blockLight($x, $y, $z);
        $this->blockLight[self::blockKey($x, $y, $z)] = $level;

        return $previous;
    }

    public function prepare(): PreparedChunkPatch
    {
        $this->primeSnapshotForPrepare();

        if ($this->chunk->revision() !== $this->baseRevision) {
            throw new MutationConflict('chunk terrain changed while mutation was being prepared');
        }
        if ($this->chunk->lightRevision()->value !== $this->baseLightRevision) {
            throw new MutationConflict('chunk light changed while mutation was being prepared');
        }

        $blocks = array_filter(
            $this->blocks,
            fn (int $stateId, int $key): bool => $stateId !== $this->blockStateIdAtKey($key),
            ARRAY_FILTER_USE_BOTH,
        );
        $biomes = array_filter(
            $this->biomes,
            fn (BiomeColumn $biome, int $key): bool => $biome->word() !== $this->biomeColumnAtKey($key)->word(),
            ARRAY_FILTER_USE_BOTH,
        );
        $extraData = array_filter(
            $this->extraData,
            fn (int $data, int $key): bool => $data !== $this->extraDataAtKey($key),
            ARRAY_FILTER_USE_BOTH,
        );
        $skyLight = array_filter(
            $this->skyLight,
            fn (int $level, int $key): bool => $level !== $this->skyLightAtKey($key),
            ARRAY_FILTER_USE_BOTH,
        );
        $blockLight = array_filter(
            $this->blockLight,
            fn (int $level, int $key): bool => $level !== $this->blockLightAtKey($key),
            ARRAY_FILTER_USE_BOTH,
        );

        $terrainChanged = $blocks !== []
            || $biomes !== []
            || $extraData !== [];
        $lightChanged = $skyLight !== []
            || $blockLight !== [];

        if ($terrainChanged && $this->baseRevision === PHP_INT_MAX) {
            throw new MutationConflict('chunk terrain revision space exhausted');
        }
        if ($lightChanged && $this->baseLightRevision === PHP_INT_MAX) {
            throw new MutationConflict('chunk light revision space exhausted');
        }

        return new PreparedChunkPatch(
            $this->chunk,
            $this->baseRevision,
            $terrainChanged ? $this->baseRevision + 1 : $this->baseRevision,
            $this->baseLightRevision,
            $lightChanged ? $this->baseLightRevision + 1 : $this->baseLightRevision,
            $blocks,
            $biomes,
            $extraData,
            $skyLight,
            $blockLight,
        );
    }

    private function blockStateIdAtKey(int $key): int
    {
        [$x, $y, $z] = self::decodeBlockKey($key);
        $snapshot = $this->snapshotForRead();
        if ($snapshot !== null) {
            return $snapshot->blockStateId($x, $y, $z)
                ?? throw new MutationConflict('snapshot block coordinates became invalid');
        }

        return $this->chunk->blockStateId($x, $y, $z);
    }

    private function biomeColumnAtKey(int $key): BiomeColumn
    {
        [$x, $z] = self::decodeColumnKey($key);
        $snapshot = $this->snapshotForRead();
        if ($snapshot !== null) {
            return $snapshot->biomeColumn($x, $z)
                ?? throw new MutationConflict('snapshot biome coordinates became invalid');
        }

        return $this->chunk->biomeColumn($x, $z);
    }

    private function extraDataAtKey(int $key): int
    {
        [$x, $y, $z] = self::decodeBlockKey($key);
        $snapshot = $this->snapshotForRead();
        if ($snapshot !== null) {
            return $snapshot->blockExtraDataAt($x, $y, $z)
                ?? throw new MutationConflict('snapshot extra-data coordinates became invalid');
        }

        return $this->chunk->blockExtraData($x, $y, $z);
    }

    private function skyLightAtKey(int $key): int
    {
        [$x, $y, $z] = self::decodeBlockKey($key);
        $snapshot = $this->snapshotForRead();
        if ($snapshot !== null) {
            return $snapshot->skyLightLevel($x, $y, $z)
                ?? throw new MutationConflict('snapshot sky-light coordinates became invalid');
        }

        return $this->chunk->skyLight($x, $y, $z);
    }

    private function blockLightAtKey(int $key): int
    {
        [$x, $y, $z] = self::decodeBlockKey($key);
        $snapshot = $this->snapshotForRead();
        if ($snapshot !== null) {
            return $snapshot->blockLightLevel($x, $y, $z)
                ?? throw new MutationConflict('snapshot block-light coordinates became invalid');
        }

        return $this->chunk->blockLight($x, $y, $z);
    }

    private function snapshotForRead(): ?ChunkSnapshot
    {
        return $this->snapshot;
    }

    private function primeSnapshotForPrepare(): void
    {
        if ($this->snapshot !== null || !$this->chunk->prefersSnapshotReads()) {
            return;
        }

        $comparisons = count($this->blocks)
            + count($this->biomes)
            + count($this->extraData)
            + count($this->skyLight)
            + count($this->blockLight);
        if ($comparisons < self::SNAPSHOT_PREPARE_THRESHOLD) {
            return;
        }

        $snapshot = $this->chunk->snapshot();
        if (
            $snapshot->revision !== $this->baseRevision
            || $snapshot->lightRevision !== $this->baseLightRevision
        ) {
            throw new MutationConflict('chunk changed while mutation snapshot was being captured');
        }

        $this->snapshot = $snapshot;
    }

    public static function blockKey(int $x, int $y, int $z): int
    {
        return ChunkCoordinateKey::block($x, $y, $z);
    }

    /** @return array{int, int, int} */
    public static function decodeBlockKey(int $key): array
    {
        return ChunkCoordinateKey::decodeBlock($key);
    }

    private static function assertLight(int $level): void
    {
        if ($level < 0 || $level > 0x0f) {
            throw new ValueError('fixed-target light level must be in range 0..15');
        }
    }

    private static function columnKey(int $x, int $z): int
    {
        return ChunkCoordinateKey::column($x, $z);
    }

    /** @return array{int, int} */
    public static function decodeColumnKey(int $key): array
    {
        return ChunkCoordinateKey::decodeColumn($key);
    }
}
