<?php

declare(strict_types=1);

namespace Cobblestone\Command\Argument;

use BackedEnum;
use Cobblestone\Command\ArgumentType;
use Cobblestone\Command\CommandParseException;
use Cobblestone\Command\CommandReader;
use UnitEnum;

final readonly class EnumArgumentType implements ArgumentType
{
    /** @param class-string<UnitEnum> $enum */
    public function __construct(private string $enum)
    {
        if (!enum_exists($enum)) {
            throw new \InvalidArgumentException("enum does not exist: {$enum}");
        }
    }

    public function parse(CommandReader $reader): UnitEnum
    {
        $cursor = $reader->cursor();
        $token = strtolower($reader->readWord());
        foreach ($this->enum::cases() as $case) {
            if (strtolower($this->token($case)) === $token) {
                return $case;
            }
        }

        throw new CommandParseException("unknown {$this->enum} value: {$token}", $cursor);
    }

    public function valueType(): string
    {
        return $this->enum;
    }

    public function suggestions(string $prefix): array
    {
        $prefix = strtolower($prefix);
        $values = array_map($this->token(...), $this->enum::cases());

        return array_values(array_filter(
            $values,
            static fn (string $value): bool => str_starts_with(strtolower($value), $prefix),
        ));
    }

    public function consumesRemaining(): bool
    {
        return false;
    }

    private function token(UnitEnum $case): string
    {
        return $case instanceof BackedEnum
            ? (string) $case->value
            : strtolower($case->name);
    }
}
