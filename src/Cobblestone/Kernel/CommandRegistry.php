<?php

declare(strict_types=1);

namespace Cobblestone\Kernel;

use Closure;
use InvalidArgumentException;
use LogicException;

/**
 * Owner-runtime command registry.
 *
 * Parsing chat/console text belongs above this type. Commands are registered by canonical name and
 * receive already-separated arguments.
 */
final class CommandRegistry
{
    /** @var array<string, Closure(list<string>): mixed> */
    private array $commands = [];

    /**
     * @param Closure(list<string>): mixed $handler
     */
    public function register(string $name, Closure $handler): void
    {
        $name = strtolower($name);
        if (preg_match('/^[a-z0-9:_-]+$/', $name) !== 1) {
            throw new InvalidArgumentException('command name must match [a-z0-9:_-]+');
        }
        if (isset($this->commands[$name])) {
            throw new LogicException("command already registered: {$name}");
        }

        $this->commands[$name] = $handler;
    }

    /**
     * @param list<string> $arguments
     */
    public function execute(string $name, array $arguments = []): mixed
    {
        $name = strtolower($name);
        $handler = $this->commands[$name] ?? null;
        if ($handler === null) {
            throw new InvalidArgumentException("unknown command: {$name}");
        }

        return $handler($arguments);
    }
}
