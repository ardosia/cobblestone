<?php

declare(strict_types=1);

namespace Cobblestone\Command;

interface ArgumentType
{
    public function parse(CommandReader $reader): mixed;

    /** @return string Builtin PHP type or class-string produced by parse(). */
    public function valueType(): string;

    /** @return list<string> */
    public function suggestions(string $prefix): array;

    public function consumesRemaining(): bool;
}
