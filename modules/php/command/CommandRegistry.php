<?php

declare(strict_types=1);

namespace Cobblestone\Command;

use Closure;
use InvalidArgumentException;
use LogicException;

final class CommandRegistry
{
    /** @var array<string, Closure(list<string>): mixed> */
    private array $commands = [];

    /** @param Closure(list<string>): mixed $handler */
    public function register(string $name, Closure $handler): void
    {
        $name = strtolower(trim($name));
        if ($name === '') {
            throw new InvalidArgumentException('command name must not be empty');
        }
        if (isset($this->commands[$name])) {
            throw new LogicException("command already registered: {$name}");
        }
        $this->commands[$name] = $handler;
    }

    /** @param list<string> $arguments */
    public function execute(string $name, array $arguments = []): mixed
    {
        $name = strtolower(trim($name));
        $handler = $this->commands[$name] ?? null;
        if ($handler === null) {
            throw new LogicException("unknown command: {$name}");
        }

        return $handler($arguments);
    }
}
