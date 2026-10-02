<?php

declare(strict_types=1);

namespace Cobblestone\Command\Argument;

use Cobblestone\Command\ArgumentType;
use Cobblestone\Command\CommandReader;

final readonly class StringArgumentType implements ArgumentType
{
    public const WORD = 'word';
    public const STRING = 'string';
    public const GREEDY = 'greedy';

    public function __construct(private string $mode)
    {
        if (!in_array($mode, [self::WORD, self::STRING, self::GREEDY], true)) {
            throw new \InvalidArgumentException("unknown string argument mode: {$mode}");
        }
    }

    public function parse(CommandReader $reader): string
    {
        return match ($this->mode) {
            self::WORD => $reader->readWord(),
            self::STRING => $reader->readString(),
            self::GREEDY => $reader->readRemaining(),
        };
    }

    public function valueType(): string
    {
        return 'string';
    }

    public function suggestions(string $prefix): array
    {
        return [];
    }

    public function consumesRemaining(): bool
    {
        return $this->mode === self::GREEDY;
    }
}
