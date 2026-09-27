<?php

declare(strict_types=1);

namespace Cobblestone\World;

use ValueError;

final class Chunk
{
    /** @var array<int, ChunkSection> */
    private array $sections;
    private string $biomes;
    private string $heightMap;

    /** @var array<int, int> */
    private array $extraData = [];

    private bool $generated = false;
    private bool $populated = false;
    private bool $lightPopulated = false;
    private int $revision = 0;

    public function __construct(
        private readonly ChunkPos $position,
        ?BiomeId $biome = null,
    ) {
        $this->sections = array_fill(0, WorldBounds::SECTION_COUNT, null);
        for ($index = 0; $index < WorldBounds::SECTION_COUNT; ++$index) {
            $this->sections[$index] = ChunkSection::air();
        }

        $biome ??= new BiomeId(1);
        $this->biomes = str_repeat(chr($biome->value), WorldBounds::CHUNK_EDGE * WorldBounds::CHUNK_EDGE);
        $this->heightMap = str_repeat("\x00", WorldBounds::CHUNK_EDGE * WorldBounds::CHUNK_EDGE);
    }

    public function position(): ChunkPos
    {
        return $this->position;
    }

    public function revision(): int
    {
        return $this->revision;
    }

    /** @internal Mutation commit primitive. */
    public function commitRevision(int $expected, int $next): void
    {
        if ($this->revision !== $expected) {
            throw new \LogicException(
                "chunk revision changed: expected {$expected}, current {$this->revision}",
            );
        }
        if ($next !== $expected + 1) {
            throw new \LogicException('chunk mutation revision must advance exactly once');
        }

        $this->revision = $next;
    }

    public function block(int $x, int $y, int $z): BlockState
    {
        self::assertBlockCoordinates($x, $y, $z);
        $section = intdiv($y, WorldBounds::SECTION_EDGE);

        return $this->sections[$section]->block($x, $y & 0x0f, $z);
    }

    public function setBlock(int $x, int $y, int $z, BlockState $state): BlockState
    {
        self::assertBlockCoordinates($x, $y, $z);
        $section = intdiv($y, WorldBounds::SECTION_EDGE);
        $previous = $this->sections[$section]->setBlock($x, $y & 0x0f, $z, $state);

        if ($previous->id !== $state->id) {
            $this->refreshHeightAfterBlockChange($x, $y, $z, $previous, $state);
        }

        return $previous;
    }

    public function skyLight(int $x, int $y, int $z): int
    {
        self::assertBlockCoordinates($x, $y, $z);
        $section = intdiv($y, WorldBounds::SECTION_EDGE);

        return $this->sections[$section]->skyLight($x, $y & 0x0f, $z);
    }

    public function setSkyLight(int $x, int $y, int $z, int $level): int
    {
        self::assertBlockCoordinates($x, $y, $z);
        $section = intdiv($y, WorldBounds::SECTION_EDGE);

        return $this->sections[$section]->setSkyLight($x, $y & 0x0f, $z, $level);
    }

    public function blockLight(int $x, int $y, int $z): int
    {
        self::assertBlockCoordinates($x, $y, $z);
        $section = intdiv($y, WorldBounds::SECTION_EDGE);

        return $this->sections[$section]->blockLight($x, $y & 0x0f, $z);
    }

    public function setBlockLight(int $x, int $y, int $z, int $level): int
    {
        self::assertBlockCoordinates($x, $y, $z);
        $section = intdiv($y, WorldBounds::SECTION_EDGE);

        return $this->sections[$section]->setBlockLight($x, $y & 0x0f, $z, $level);
    }

    public function biome(int $x, int $z): BiomeId
    {
        return new BiomeId(ord($this->biomes[self::columnIndex($x, $z)]));
    }

    public function setBiome(int $x, int $z, BiomeId $biome): BiomeId
    {
        $index = self::columnIndex($x, $z);
        $previous = new BiomeId(ord($this->biomes[$index]));
        $this->biomes[$index] = chr($biome->value);

        return $previous;
    }

    public function highestBlockAt(int $x, int $z): int
    {
        self::columnIndex($x, $z);

        for ($y = WorldBounds::MAX_Y; $y >= WorldBounds::MIN_Y; --$y) {
            if (!$this->block($x, $y, $z)->isAir()) {
                return $y;
            }
        }

        return 0;
    }

    public function heightMap(int $x, int $z): int
    {
        return ord($this->heightMap[self::columnIndex($x, $z)]);
    }

    public function recalculateHeightMap(): void
    {
        for ($z = 0; $z < WorldBounds::CHUNK_EDGE; ++$z) {
            for ($x = 0; $x < WorldBounds::CHUNK_EDGE; ++$x) {
                $this->heightMap[self::columnIndex($x, $z)] = chr($this->highestBlockAt($x, $z));
            }
        }
    }

    public function blockExtraData(int $x, int $y, int $z): int
    {
        self::assertBlockCoordinates($x, $y, $z);

        return $this->extraData[self::extraDataKey($x, $y, $z)] ?? 0;
    }

    public function setBlockExtraData(int $x, int $y, int $z, int $data): int
    {
        self::assertBlockCoordinates($x, $y, $z);
        if ($data < 0 || $data > 0xffff) {
            throw new ValueError('fixed-target block extra data must be in range 0..65535');
        }

        $key = self::extraDataKey($x, $y, $z);
        $previous = $this->extraData[$key] ?? 0;
        if ($data === 0) {
            unset($this->extraData[$key]);
        } else {
            $this->extraData[$key] = $data;
        }

        return $previous;
    }

    /** @return array<int, int> */
    public function extraData(): array
    {
        return $this->extraData;
    }

    public function isGenerated(): bool
    {
        return $this->generated;
    }

    public function markGenerated(bool $generated = true): void
    {
        $this->generated = $generated;
    }

    public function isPopulated(): bool
    {
        return $this->populated;
    }

    public function markPopulated(bool $populated = true): void
    {
        $this->populated = $populated;
    }

    public function isLightPopulated(): bool
    {
        return $this->lightPopulated;
    }

    public function markLightPopulated(bool $lightPopulated = true): void
    {
        $this->lightPopulated = $lightPopulated;
    }

    private function refreshHeightAfterBlockChange(
        int $x,
        int $y,
        int $z,
        BlockState $previous,
        BlockState $next,
    ): void {
        $current = $this->heightMap($x, $z);
        if (!$next->isAir() && ($previous->isAir() || $y >= $current)) {
            $this->heightMap[self::columnIndex($x, $z)] = chr($y);
            return;
        }

        if (!$previous->isAir() && $next->isAir() && $y >= $current) {
            $this->heightMap[self::columnIndex($x, $z)] = chr($this->highestBlockAt($x, $z));
        }
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