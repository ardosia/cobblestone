<?php

declare(strict_types=1);

namespace Cobblestone\World;

use ValueError;

final class ChunkSection
{
    public const VOLUME = 16 * 16 * 16;
    private const NIBBLE_BYTES = self::VOLUME / 2;

    private string $blockIds;
    private string $blockData;
    private string $skyLight;
    private string $blockLight;

    private function __construct(BlockState $state)
    {
        $this->blockIds = str_repeat(chr($state->id), self::VOLUME);
        $nibble = chr(($state->data << 4) | $state->data);
        $this->blockData = str_repeat($nibble, self::NIBBLE_BYTES);
        $this->skyLight = str_repeat("\x00", self::NIBBLE_BYTES);
        $this->blockLight = str_repeat("\x00", self::NIBBLE_BYTES);
    }

    public static function filled(BlockState $state): self
    {
        return new self($state);
    }

    public static function air(): self
    {
        return new self(BlockState::air());
    }

    public function block(int $x, int $y, int $z): BlockState
    {
        $index = self::index($x, $y, $z);

        return new BlockState(
            ord($this->blockIds[$index]),
            self::readNibble($this->blockData, $index),
        );
    }

    public function setBlock(int $x, int $y, int $z, BlockState $state): BlockState
    {
        $index = self::index($x, $y, $z);
        $previous = new BlockState(
            ord($this->blockIds[$index]),
            self::readNibble($this->blockData, $index),
        );

        $this->blockIds[$index] = chr($state->id);
        self::writeNibble($this->blockData, $index, $state->data);

        return $previous;
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
