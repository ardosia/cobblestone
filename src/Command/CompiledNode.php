<?php

declare(strict_types=1);

namespace Cobblestone\Command;

use Closure;

/** @internal */
final readonly class CompiledNode
{
    /**
     * @param list<CompiledNode> $children
     * @param list<Closure(Command): bool> $requirements
     */
    public function __construct(
        public Node $node,
        public array $children,
        public ?Closure $invoke,
        public array $requirements,
    ) {}
}
