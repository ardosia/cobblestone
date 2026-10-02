<?php

declare(strict_types=1);

namespace Cobblestone\Command\Argument;

use Cobblestone\Command\ArgumentType;
use Cobblestone\Command\CommandParseException;
use Cobblestone\Command\CommandReader;

final readonly class IntegerArgumentType implements ArgumentType
{
    public function __construct(
        private int $min = PHP_INT_MIN,
        private int $max = PHP_INT_MAX,
    ) {
        if ($min > $max) {
            throw new \InvalidArgumentException('integer minimum exceeds maximum');
        }
    }

    public function parse(CommandReader $reader): int
    {
        $cursor = $reader->cursor();
        $token = $reader->readWord();
        if (preg_match('/^-?\d+$/D', $token) !== 1) {
            throw new CommandParseException("expected integer, got "{$token}"", $cursor);
        }

        $value = filter_var($token, FILTER_VALIDATE_INT);
        if ($value === false || $value < $this->min || $value > $this->max) {
            throw new CommandParseException(
                "integer {$token} is outside {$this->min}..{$this->max}",
                $cursor,
            );
        }

        return $value;
    }

    public function valueType(): string
    {
        return 'int';
    }

    public function suggestions(string $prefix): array
    {
        return [];
    }

    public function consumesRemaining(): bool
    {
        return false;
    }
}
