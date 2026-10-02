<?php

declare(strict_types=1);

namespace Cobblestone\Command;

use Cobblestone\Command\Argument\BooleanArgumentType;
use Cobblestone\Command\Argument\EnumArgumentType;
use Cobblestone\Command\Argument\IntegerArgumentType;
use Cobblestone\Command\Argument\StringArgumentType;
use UnitEnum;

/** @param list<string> $aliases */
function literal(string $name, array $aliases = []): Literal
{
    return new Literal($name, $aliases);
}

function word(string $name): Argument
{
    return new Argument($name, new StringArgumentType(StringArgumentType::WORD));
}

function string(string $name): Argument
{
    return new Argument($name, new StringArgumentType(StringArgumentType::STRING));
}

function greedyString(string $name): Argument
{
    return new Argument($name, new StringArgumentType(StringArgumentType::GREEDY));
}

function integer(
    string $name,
    int $min = PHP_INT_MIN,
    int $max = PHP_INT_MAX,
): Argument {
    return new Argument($name, new IntegerArgumentType($min, $max));
}

function boolean(string $name): Argument
{
    return new Argument($name, new BooleanArgumentType());
}

/** @param class-string<UnitEnum> $enum */
function enumArg(string $name, string $enum): Argument
{
    return new Argument($name, new EnumArgumentType($enum));
}
