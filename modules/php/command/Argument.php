<?php

declare(strict_types=1);

namespace Cobblestone\Command;

use Closure;
use InvalidArgumentException;

final class Argument extends Node
{
    private ?Closure $suggestions = null;

    public function __construct(
        private readonly string $name,
        private readonly ArgumentType $type,
    ) {
        if ($name === '' || preg_match('/\s/', $name) === 1) {
            throw new InvalidArgumentException('argument name must be one non-empty token');
        }
    }

    public function name(): string
    {
        return $this->name;
    }

    public function type(): ArgumentType
    {
        return $this->type;
    }

    /** @param Closure(Command, string): iterable<string> $provider */
    #[\NoDiscard]
    public function suggests(Closure $provider): self
    {
        $copy = clone $this;
        $copy->suggestions = $provider;

        return $copy;
    }

    public function suggestionProvider(): ?Closure
    {
        return $this->suggestions;
    }
}
