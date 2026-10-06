<?php

declare(strict_types=1);

namespace Cobblestone\Command\Argument;

use Cobblestone\Command\ArgumentType;
use Cobblestone\Command\CommandParseException;
use Cobblestone\Command\CommandReader;

final readonly class BooleanArgumentType implements ArgumentType
{
    public function parse(CommandReader $reader): bool
    {
        $cursor = $reader->cursor();

        return match (strtolower($reader->readWord())) {
            'true' => true,
            'false' => false,
            default => throw new CommandParseException(
                'expected boolean true or false',
                $cursor,
            ),
        };
    }

    public function valueType(): string
    {
        return 'bool';
    }

    public function suggestions(string $prefix): array
    {
        $prefix = strtolower($prefix);

        return array_values(array_filter(
            ['true', 'false'],
            static fn(string $value): bool => str_starts_with($value, $prefix),
        ));
    }

    public function consumesRemaining(): bool
    {
        return false;
    }
}
