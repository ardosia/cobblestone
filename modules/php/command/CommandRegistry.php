<?php

declare(strict_types=1);

namespace Cobblestone\Command;

use Closure;
use InvalidArgumentException;
use LogicException;

final class CommandRegistry
{
    /** @var array<string, array{id: int, handler: Closure(list<string>): mixed}> */
    private array $commands = [];

    private int $nextBindingId = 1;

    /** @param Closure(list<string>): mixed $handler */
    public function register(string $name, Closure $handler): CommandBinding
    {
        $name = self::normalize($name);
        if (isset($this->commands[$name])) {
            throw new LogicException("command already registered: {$name}");
        }

        $id = $this->nextBindingId++;
        $this->commands[$name] = ['id' => $id, 'handler' => $handler];

        return new CommandBinding(
            fn (): bool => $this->remove($name, $id),
        );
    }

    /** @param list<string> $arguments */
    public function execute(string $name, array $arguments = []): mixed
    {
        $name = self::normalize($name);
        $command = $this->commands[$name] ?? null;
        if ($command === null) {
            throw new LogicException("unknown command: {$name}");
        }

        return ($command['handler'])($arguments);
    }

    private function remove(string $name, int $id): bool
    {
        if (($this->commands[$name]['id'] ?? null) !== $id) {
            return false;
        }

        unset($this->commands[$name]);

        return true;
    }

    private static function normalize(string $name): string
    {
        $name = strtolower(trim($name));
        if ($name === '') {
            throw new InvalidArgumentException('command name must not be empty');
        }

        return $name;
    }
}
