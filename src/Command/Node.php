<?php

declare(strict_types=1);

namespace Cobblestone\Command;

use Closure;

abstract class Node
{
    /** @var list<Node> */
    private array $children = [];

    private ?Closure $handler = null;

    /** @var list<Closure(Command): bool> */
    private array $requirements = [];

    /** @return list<Node> */
    final public function children(): array
    {
        return $this->children;
    }

    final public function handler(): ?Closure
    {
        return $this->handler;
    }

    /** @return list<Closure(Command): bool> */
    final public function requirements(): array
    {
        return $this->requirements;
    }

    #[\NoDiscard]
    final public function then(Node ...$children): static
    {
        if ($children === []) {
            return $this;
        }

        $copy = clone $this;
        $copy->children = [...$this->children, ...$children];

        return $copy;
    }

    #[\NoDiscard]
    final public function executes(Closure $handler): static
    {
        $copy = clone $this;
        $copy->handler = $handler;

        return $copy;
    }

    /** @param Closure(Command): bool $requirement */
    #[\NoDiscard]
    final public function requires(Closure $requirement): static
    {
        $copy = clone $this;
        $copy->requirements = [...$this->requirements, $requirement];

        return $copy;
    }
}
