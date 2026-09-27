<?php

declare(strict_types=1);

namespace Cobblestone\World\Mutation;

use Cobblestone\World\BiomeId;
use Cobblestone\World\BlockState;
use Cobblestone\World\Chunk;
use ValueError;

final class ChunkPatch
{
    private readonly int $baseRevision;

    /** @var array<int, BlockState> */
    private array $blocks = [];

    /** @var array<int, BiomeId> */
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
    }

    public function chunk(): Chunk
    {
        return $this->chunk;
    }

    public function baseRevision(): int
    {
        return $this->baseRevision;
    }

    public function block(int $x, int $y, int $z): BlockState
    {
        $key = self::blockKey($x, $y, $z);

        return $this->blocks[$key] ?? $this->chunk->block($x, $y, $z);
    }

    public function setBlock(int $x, int $y, int $z, BlockState $state): BlockState
    {
        $previous = $this->block($x, $y, $z);
        $this->blocks[self::blockKey($x, $y, $z)] = $state;

        return $previous;
    }

    public function biome(int $x, int $z): BiomeId
    {
        $key = self::columnKey($x, $z);

        return $this->biomes[$key] ?? $this->chunk->biome($x, $z);
    }

    public function setBiome(int $x, int $z, BiomeId $biome): BiomeId
    {
        $previous = $this->biome($x, $z);
        $this->biomes[self::columnKey($x, $z)] = $biome;

        return $previous;
    }

    public function blockExtraData(int $x, int $y, int $z): int
    {
        $key = self::blockKey($x, $y, $z);

        return $this->extraData[$key] ?? $this->chunk->blockExtraData($x, $y, $z);
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

        return $this->skyLight[$key] ?? $this->chunk->skyLight($x, $y, $z);
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

        return $this->blockLight[$key] ?? $this->chunk->blockLight($x, $y, $z);
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
        if ($this->chunk->revision() !== $this->baseRevision) {
            throw new MutationConflict('chunk changed while mutation was being prepared');
        }

        $blocks = array_filter(
            $this->blocks,
            fn (BlockState $state, int $key): bool => $state->fullId() !== $this->blockAtKey($key)->fullId(),
            ARRAY_FILTER_USE_BOTH,
        );
        $biomes = array_filter(
            $this->biomes,
            fn (BiomeId $biome, int $key): bool => $biome->value !== $this->biomeAtKey($key)->value,
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

        $changed = $blocks !== []
            || $biomes !== []
            || $extraData !== []
            || $skyLight !== []
            || $blockLight !== [];

        if ($changed && $this->baseRevision === PHP_INT_MAX) {
            throw new MutationConflict('chunk revision space exhausted');
        }

        return new PreparedChunkPatch(
            $this->chunk,
            $this->baseRevision,
            $changed ? $this->baseRevision + 1 : $this->baseRevision,
            $blocks,
            $biomes,
            $extraData,
            $skyLight,
            $blockLight,
        );
    }

    private function blockAtKey(int $key): BlockState
    {
        [$x, $y, $z] = self::decodeBlockKey($key);

        return $this->chunk->block($x, $y, $z);
    }

    private function biomeAtKey(int $key): BiomeId
    {
        [$x, $z] = self::decodeColumnKey($key);

        return $this->chunk->biome($x, $z);
    }

    private function extraDataAtKey(int $key): int
    {
        [$x, $y, $z] = self::decodeBlockKey($key);

        return $this->chunk->blockExtraData($x, $y, $z);
    }

    private function skyLightAtKey(int $key): int
    {
        [$x, $y, $z] = self::decodeBlockKey($key);

        return $this->chunk->skyLight($x, $y, $z);
    }

    private function blockLightAtKey(int $key): int
    {
        [$x, $y, $z] = self::decodeBlockKey($key);

        return $this->chunk->blockLight($x, $y, $z);
    }

    public static function blockKey(int $x, int $y, int $z): int
    {
        return ($y << 8) | ($z << 4) | $x;
    }

    /** @return array{int, int, int} */
    public static function decodeBlockKey(int $key): array
    {
        return [$key & 0x0f, ($key >> 8) & 0x7f, ($key >> 4) & 0x0f];
    }

    private static function assertLight(int $level): void
    {
        if ($level < 0 || $level > 0x0f) {
            throw new ValueError('fixed-target light level must be in range 0..15');
        }
    }

    private static function columnKey(int $x, int $z): int
    {
        return ($z << 4) | $x;
    }

    /** @return array{int, int} */
    public static function decodeColumnKey(int $key): array
    {
        return [$key & 0x0f, ($key >> 4) & 0x0f];
    }
}