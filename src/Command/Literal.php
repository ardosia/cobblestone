<?php

declare(strict_types=1);

namespace Cobblestone\Command;

use InvalidArgumentException;

final class Literal extends Node
{
    private string $name;

    /** @var list<string> */
    private array $aliases;

    /** @param list<string> $aliases */
    public function __construct(string $name, array $aliases = [])
    {
        $this->name = self::normalize($name);
        $this->aliases = self::normalizeAliases($this->name, $aliases);
    }

    public function name(): string
    {
        return $this->name;
    }

    /** @return list<string> */
    public function aliasNames(): array
    {
        return $this->aliases;
    }

    /** @return list<string> */
    public function names(): array
    {
        return [$this->name, ...$this->aliases];
    }

    public function matches(string $token): bool
    {
        return in_array(strtolower($token), $this->names(), true);
    }

    #[\NoDiscard]
    public function aliases(string ...$aliases): self
    {
        $copy = clone $this;
        $copy->aliases = self::normalizeAliases(
            $this->name,
            [...$this->aliases, ...$aliases],
        );

        return $copy;
    }

    private static function normalize(string $name): string
    {
        $name = strtolower(trim($name));
        if ($name === '' || preg_match('/\s/', $name) === 1) {
            throw new InvalidArgumentException('literal must be one non-empty token');
        }

        return $name;
    }

    /** @param list<string> $aliases @return list<string> */
    private static function normalizeAliases(string $name, array $aliases): array
    {
        $normalized = [];
        foreach ($aliases as $alias) {
            $alias = self::normalize($alias);
            if ($alias === $name || in_array($alias, $normalized, true)) {
                continue;
            }
            $normalized[] = $alias;
        }

        return $normalized;
    }
}
