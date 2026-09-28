<?php

declare(strict_types=1);

namespace Cobblestone\World;

use ValueError;

final class ChunkSection
{
    public const VOLUME = 16 * 16 * 16;
    private const NIBBLE_BYTES = self::VOLUME / 2;
    private const LAYER_BLOCK_BYTES = 16 * 16;
    private const LAYER_NIBBLE_BYTES = self::LAYER_BLOCK_BYTES / 2;

    private string $blockIds;
    private string $blockData;
    private string $skyLight;
    private string $blockLight;
    private string $columnHeights;

    private function __construct(int $stateId)
    {
        BlockStateId::assert($stateId);
        $id = $stateId >> 4;
        $data = $stateId & 0x0f;

        $this->blockIds = str_repeat(chr($id), self::VOLUME);
        $nibble = chr(($data << 4) | $data);
        $this->blockData = str_repeat($nibble, self::NIBBLE_BYTES);
        $this->skyLight = str_repeat("\x00", self::NIBBLE_BYTES);
        $this->blockLight = str_repeat("\x00", self::NIBBLE_BYTES);
        $height = $id === 0 ? 0xff : WorldBounds::SECTION_EDGE - 1;
        $this->columnHeights = str_repeat(chr($height), WorldBounds::CHUNK_EDGE * WorldBounds::CHUNK_EDGE);
    }

    public static function air(): self
    {
        return new self(0);
    }

    public function blockStateId(int $x, int $y, int $z): int
    {
        $index = self::index($x, $y, $z);

        return (ord($this->blockIds[$index]) << 4) | self::readNibble($this->blockData, $index);
    }

    public function block(int $x, int $y, int $z): BlockState
    {
        return BlockState::fromId($this->blockStateId($x, $y, $z));
    }

    public function setBlockStateId(int $x, int $y, int $z, int $stateId): int
    {
        BlockStateId::assert($stateId);
        $index = self::index($x, $y, $z);
        $previous = (ord($this->blockIds[$index]) << 4) | self::readNibble($this->blockData, $index);
        if ($previous === $stateId) {
            return $previous;
        }

        $this->blockIds[$index] = chr($stateId >> 4);
        self::writeNibble($this->blockData, $index, $stateId & 0x0f);

        $previousAir = ($previous >> 4) === 0;
        $nextAir = ($stateId >> 4) === 0;
        if ($previousAir !== $nextAir) {
            $this->refreshColumnHeight($x, $y, $z, $previousAir, $nextAir);
        }

        return $previous;
    }

    public function setBlock(int $x, int $y, int $z, BlockState $state): BlockState
    {
        return BlockState::fromId($this->setBlockStateId($x, $y, $z, $state->fullId()));
    }

    /**
     * Fills complete local Y layers using a scalar state token.
     *
     * @internal Generator/native-fallback initialization primitive.
     */
    public function fillLayers(int $startY, int $count, int $stateId): void
    {
        BlockStateId::assert($stateId);
        if ($startY < 0 || $startY > WorldBounds::SECTION_EDGE) {
            throw new ValueError('chunk-section layer start must be in range 0..16');
        }
        if ($count < 0 || $startY + $count > WorldBounds::SECTION_EDGE) {
            throw new ValueError('chunk-section layer range must stay inside 0..16');
        }
        if ($count === 0) {
            return;
        }

        $id = $stateId >> 4;
        $data = $stateId & 0x0f;
        $blockOffset = $startY * self::LAYER_BLOCK_BYTES;
        $blockLength = $count * self::LAYER_BLOCK_BYTES;
        $this->blockIds = substr_replace(
            $this->blockIds,
            str_repeat(chr($id), $blockLength),
            $blockOffset,
            $blockLength,
        );

        $nibbleOffset = $startY * self::LAYER_NIBBLE_BYTES;
        $nibbleLength = $count * self::LAYER_NIBBLE_BYTES;
        $nibble = chr(($data << 4) | $data);
        $this->blockData = substr_replace(
            $this->blockData,
            str_repeat($nibble, $nibbleLength),
            $nibbleOffset,
            $nibbleLength,
        );

        if ($id !== 0) {
            $top = $startY + $count - 1;
            for ($column = 0; $column < WorldBounds::CHUNK_EDGE * WorldBounds::CHUNK_EDGE; ++$column) {
                $current = ord($this->columnHeights[$column]);
                if ($current === 0xff || $top > $current) {
                    $this->columnHeights[$column] = chr($top);
                }
            }
            return;
        }

        $this->recalculateColumnHeights();
    }

    public function highestBlockAt(int $x, int $z): ?int
    {
        $height = ord($this->columnHeights[self::columnIndex($x, $z)]);

        return $height === 0xff ? null : $height;
    }

    public function skyLight(int $x, int $y, int $z): int
    {
        return self::readNibble($this->skyLight, self::index($x, $y, $z));
    }

    public function setSkyLight(int $x, int $y, int $z, int $level): int
    {
        self::assertLight($level);
        $index = self::index($x, $y, $z);
        $previous = self::readNibble($this->skyLight, $index);
        self::writeNibble($this->skyLight, $index, $level);

        return $previous;
    }

    /**
     * Fills every semantic sky-light cell from the given local Y through the section top.
     *
     * @internal Generator initialization primitive.
     */
    public function fillSkyLightFrom(int $y, int $level): void
    {
        if ($y < 0 || $y > WorldBounds::SECTION_EDGE) {
            throw new ValueError('chunk-section sky-light fill y must be in range 0..16');
        }
        self::assertLight($level);
        if ($y === WorldBounds::SECTION_EDGE) {
            return;
        }

        $offset = $y * self::LAYER_NIBBLE_BYTES;
        $byte = chr(($level << 4) | $level);
        $this->skyLight = substr($this->skyLight, 0, $offset)
            . str_repeat($byte, self::NIBBLE_BYTES - $offset);
    }

    public function blockLight(int $x, int $y, int $z): int
    {
        return self::readNibble($this->blockLight, self::index($x, $y, $z));
    }

    public function setBlockLight(int $x, int $y, int $z, int $level): int
    {
        self::assertLight($level);
        $index = self::index($x, $y, $z);
        $previous = self::readNibble($this->blockLight, $index);
        self::writeNibble($this->blockLight, $index, $level);

        return $previous;
    }

    /** @internal Bulk immutable world/native boundary. */
    public function snapshot(): ChunkSectionSnapshot
    {
        return new ChunkSectionSnapshot(
            $this->blockIds,
            $this->blockData,
            $this->skyLight,
            $this->blockLight,
        );
    }

    private function refreshColumnHeight(
        int $x,
        int $y,
        int $z,
        bool $previousAir,
        bool $nextAir,
    ): void {
        $column = self::columnIndex($x, $z);
        $current = ord($this->columnHeights[$column]);

        if (!$nextAir) {
            if ($current === 0xff || $y > $current) {
                $this->columnHeights[$column] = chr($y);
            }
            return;
        }

        if ($previousAir || $current !== $y) {
            return;
        }

        for ($candidate = $y - 1; $candidate >= 0; --$candidate) {
            if (ord($this->blockIds[self::index($x, $candidate, $z)]) !== 0) {
                $this->columnHeights[$column] = chr($candidate);
                return;
            }
        }

        $this->columnHeights[$column] = "\xff";
    }

    private function recalculateColumnHeights(): void
    {
        for ($z = 0; $z < WorldBounds::CHUNK_EDGE; ++$z) {
            for ($x = 0; $x < WorldBounds::CHUNK_EDGE; ++$x) {
                $height = 0xff;
                for ($y = WorldBounds::SECTION_EDGE - 1; $y >= 0; --$y) {
                    if (ord($this->blockIds[self::index($x, $y, $z)]) !== 0) {
                        $height = $y;
                        break;
                    }
                }
                $this->columnHeights[self::columnIndex($x, $z)] = chr($height);
            }
        }
    }

    private static function columnIndex(int $x, int $z): int
    {
        if (!WorldBounds::containsLocal($x) || !WorldBounds::containsLocal($z)) {
            throw new ValueError('chunk-section column coordinates must be in range 0..15');
        }

        return ($z << 4) | $x;
    }

    private static function index(int $x, int $y, int $z): int
    {
        if (
            !WorldBounds::containsLocal($x)
            || !WorldBounds::containsLocal($y)
            || !WorldBounds::containsLocal($z)
        ) {
            throw new ValueError('chunk-section coordinates must be in range 0..15');
        }

        return ($y << 8) | ($z << 4) | $x;
    }

    private static function readNibble(string $bytes, int $index): int
    {
        $value = ord($bytes[$index >> 1]);

        return ($index & 1) === 0 ? $value & 0x0f : ($value >> 4) & 0x0f;
    }

    private static function writeNibble(string &$bytes, int $index, int $value): void
    {
        $byteIndex = $index >> 1;
        $current = ord($bytes[$byteIndex]);
        $next = ($index & 1) === 0
            ? ($current & 0xf0) | $value
            : ($current & 0x0f) | ($value << 4);

        $bytes[$byteIndex] = chr($next);
    }

    private static function assertLight(int $level): void
    {
        if ($level < 0 || $level > 0x0f) {
            throw new ValueError('fixed-target light level must be in range 0..15');
        }
    }
}
