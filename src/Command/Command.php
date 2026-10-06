<?php

declare(strict_types=1);

namespace Cobblestone\Command;

use OutOfBoundsException;

final readonly class Command
{
    /** @param array<string, mixed> $arguments */
    public function __construct(
        private string $input,
        private mixed $source,
        private array $arguments,
    ) {}

    public function input(): string
    {
        return $this->input;
    }

    public function source(): mixed
    {
        return $this->source;
    }

    /** @return array<string, mixed> */
    public function arguments(): array
    {
        return $this->arguments;
    }

    public function hasArgument(string $name): bool
    {
        return array_key_exists($name, $this->arguments);
    }

    public function argument(string $name): mixed
    {
        if (!array_key_exists($name, $this->arguments)) {
            throw new OutOfBoundsException("command argument is unavailable: {$name}");
        }

        return $this->arguments[$name];
    }
}
