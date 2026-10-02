<?php

declare(strict_types=1);

namespace Cobblestone\Command;

use Closure;
use ReflectionFunction;
use ReflectionNamedType;
use ReflectionParameter;

final class CommandRegistry
{
    /** @var array<int, array{root: CompiledNode, names: list<string>}> */
    private array $registrations = [];

    /** @var array<string, int> */
    private array $roots = [];

    private int $nextBindingId = 1;

    public function register(Literal $root): CommandBinding
    {
        $names = $root->names();
        foreach ($names as $name) {
            if (isset($this->roots[$name])) {
                throw new CommandDefinitionException("command root is already registered: {$name}");
            }
        }

        $compiled = $this->compile($root, []);
        $id = $this->nextBindingId++;
        $this->registrations[$id] = ['root' => $compiled, 'names' => $names];
        foreach ($names as $name) {
            $this->roots[$name] = $id;
        }

        return new CommandBinding(fn (): bool => $this->remove($id));
    }

    public function execute(string $input, mixed $source = null): mixed
    {
        $input = self::normalizeInput($input);
        if ($input === '') {
            throw new CommandParseException('command input is empty');
        }

        $reader = new CommandReader($input);
        $rootToken = strtolower($reader->readWord());
        $id = $this->roots[$rootToken] ?? null;
        if ($id === null) {
            throw new CommandParseException("unknown command: {$rootToken}", 0);
        }

        $root = $this->registrations[$id]['root'];
        $arguments = [];
        $this->assertRequirements($root, $input, $source, $arguments, 0);

        return $this->executeFrom($root, $reader, $input, $source, $arguments);
    }

    /** @return list<string> */
    public function suggest(string $input, mixed $source = null): array
    {
        $input = ltrim($input);
        if (str_starts_with($input, '/')) {
            $input = substr($input, 1);
        }

        if ($input === '' || preg_match('/\s/', $input) !== 1) {
            return $this->rootSuggestions(strtolower($input));
        }

        $trailingWhitespace = preg_match('/\s$/', $input) === 1;
        $trimmed = trim($input);
        if ($trimmed === '') {
            return $this->rootSuggestions('');
        }

        /** @var list<string> $parts */
        $parts = preg_split('/\s+/', $trimmed, -1, PREG_SPLIT_NO_EMPTY) ?: [];
        $prefix = $trailingWhitespace
            ? ''
            : strtolower((string) array_pop($parts));
        if ($parts === []) {
            return $this->rootSuggestions($prefix);
        }

        $rootToken = strtolower((string) array_shift($parts));
        $id = $this->roots[$rootToken] ?? null;
        if ($id === null) {
            return $this->rootSuggestions($rootToken);
        }

        $node = $this->registrations[$id]['root'];
        $arguments = [];
        foreach ($parts as $token) {
            $matched = $this->matchSuggestionToken($node, $token, $arguments);
            if ($matched === null) {
                return [];
            }
            [$node, $arguments] = $matched;
        }

        $command = new Command(self::normalizeInput($input), $source, $arguments);
        $suggestions = [];
        foreach ($node->children as $child) {
            if (!$this->requirementsPass($child, $command)) {
                continue;
            }

            if ($child->node instanceof Literal) {
                foreach ($child->node->names() as $name) {
                    if (str_starts_with($name, $prefix)) {
                        $suggestions[] = $name;
                    }
                }
                continue;
            }

            if (!$child->node instanceof Argument) {
                continue;
            }

            $provider = $child->node->suggestionProvider();
            $values = $provider === null
                ? $child->node->type()->suggestions($prefix)
                : $provider($command, $prefix);
            foreach ($values as $value) {
                if (is_string($value) && str_starts_with(strtolower($value), $prefix)) {
                    $suggestions[] = $value;
                }
            }
        }

        $suggestions = array_values(array_unique($suggestions));
        sort($suggestions, SORT_STRING);

        return $suggestions;
    }

    /**
     * @param array<string, string> $argumentTypes
     */
    private function compile(Node $node, array $argumentTypes): CompiledNode
    {
        if ($node instanceof Argument) {
            if (isset($argumentTypes[$node->name()])) {
                throw new CommandDefinitionException(
                    "duplicate command argument: {$node->name()}",
                );
            }
            $argumentTypes[$node->name()] = $node->type()->valueType();
            if ($node->type()->consumesRemaining() && $node->children() !== []) {
                throw new CommandDefinitionException(
                    "greedy argument <{$node->name()}> cannot have children",
                );
            }
        }

        $this->validateChildren($node);

        $children = [];
        foreach ($node->children() as $child) {
            $children[] = $this->compile($child, $argumentTypes);
        }

        $handler = $node->handler();
        $invoke = $handler === null
            ? null
            : $this->compileHandler($handler, $argumentTypes);

        return new CompiledNode(
            $node,
            $children,
            $invoke,
            $node->requirements(),
        );
    }

    private function validateChildren(Node $node): void
    {
        $literalNames = [];
        $argument = null;

        foreach ($node->children() as $child) {
            if ($child instanceof Literal) {
                foreach ($child->names() as $name) {
                    if (isset($literalNames[$name])) {
                        throw new CommandDefinitionException(
                            "duplicate sibling literal or alias: {$name}",
                        );
                    }
                    $literalNames[$name] = true;
                }
                continue;
            }

            if ($child instanceof Argument) {
                if ($argument !== null) {
                    throw new CommandDefinitionException(
                        "ambiguous argument siblings <{$argument}> and <{$child->name()}>",
                    );
                }
                $argument = $child->name();
            }
        }
    }

    /**
     * @param array<string, string> $argumentTypes
     * @return Closure(Command): mixed
     */
    private function compileHandler(Closure $handler, array $argumentTypes): Closure
    {
        $reflection = new ReflectionFunction($handler);
        $plan = [];

        foreach ($reflection->getParameters() as $parameter) {
            if ($parameter->isVariadic()) {
                throw new CommandDefinitionException(
                    'command handlers cannot be variadic',
                );
            }

            $type = $parameter->getType();
            if (!$type instanceof ReflectionNamedType) {
                throw new CommandDefinitionException(
                    "handler parameter ${$parameter->getName()} must have one named type",
                );
            }

            $typeName = $type->getName();
            if ($typeName === Command::class) {
                $plan[] = ['kind' => 'command'];
                continue;
            }

            $argumentName = $parameter->getName();
            $produced = $argumentTypes[$argumentName] ?? null;
            if ($produced === null) {
                throw new CommandDefinitionException(
                    "handler parameter ${$argumentName} has no command argument",
                );
            }
            if (!$this->parameterAccepts($parameter, $produced)) {
                throw new CommandDefinitionException(
                    "argument <{$argumentName}> produces {$produced}, handler expects {$typeName}",
                );
            }

            $plan[] = ['kind' => 'argument', 'name' => $argumentName];
        }

        return static function (Command $command) use ($handler, $plan): mixed {
            $values = [];
            foreach ($plan as $entry) {
                $values[] = $entry['kind'] === 'command'
                    ? $command
                    : $command->argument($entry['name']);
            }

            return $handler(...$values);
        };
    }

    private function parameterAccepts(
        ReflectionParameter $parameter,
        string $produced,
    ): bool {
        $type = $parameter->getType();
        if (!$type instanceof ReflectionNamedType) {
            return false;
        }

        if ($type->isBuiltin()) {
            return $type->getName() === $produced;
        }

        return $type->getName() === $produced
            || is_a($produced, $type->getName(), true);
    }

    /** @param array<string, mixed> $arguments */
    private function executeFrom(
        CompiledNode $node,
        CommandReader $reader,
        string $input,
        mixed $source,
        array $arguments,
    ): mixed {
        if ($reader->atEnd()) {
            if ($node->invoke === null) {
                throw new CommandParseException(
                    'incomplete command',
                    $reader->cursor(),
                );
            }

            return ($node->invoke)(new Command($input, $source, $arguments));
        }

        $literalProbe = $reader->copy();
        $token = $literalProbe->readWord();
        foreach ($node->children as $child) {
            if (
                !$child->node instanceof Literal
                || !$child->node->matches($token)
            ) {
                continue;
            }

            $this->assertRequirements(
                $child,
                $input,
                $source,
                $arguments,
                $reader->cursor(),
            );

            return $this->executeFrom(
                $child,
                $literalProbe,
                $input,
                $source,
                $arguments,
            );
        }

        foreach ($node->children as $child) {
            if (!$child->node instanceof Argument) {
                continue;
            }

            $argumentReader = $reader->copy();
            $value = $child->node->type()->parse($argumentReader);
            $nextArguments = $arguments;
            $nextArguments[$child->node->name()] = $value;
            $this->assertRequirements(
                $child,
                $input,
                $source,
                $nextArguments,
                $reader->cursor(),
            );

            return $this->executeFrom(
                $child,
                $argumentReader,
                $input,
                $source,
                $nextArguments,
            );
        }

        throw new CommandParseException(
            "unexpected token: {$token}",
            $reader->cursor(),
        );
    }

    /**
     * @param array<string, mixed> $arguments
     * @return array{CompiledNode, array<string, mixed>}|null
     */
    private function matchSuggestionToken(
        CompiledNode $node,
        string $token,
        array $arguments,
    ): ?array {
        foreach ($node->children as $child) {
            if (
                $child->node instanceof Literal
                && $child->node->matches($token)
            ) {
                return [$child, $arguments];
            }
        }

        foreach ($node->children as $child) {
            if (
                !$child->node instanceof Argument
                || $child->node->type()->consumesRemaining()
            ) {
                continue;
            }

            $reader = new CommandReader($token);
            try {
                $value = $child->node->type()->parse($reader);
            } catch (CommandParseException) {
                continue;
            }
            if (!$reader->atEnd()) {
                continue;
            }

            $arguments[$child->node->name()] = $value;

            return [$child, $arguments];
        }

        return null;
    }

    /** @param array<string, mixed> $arguments */
    private function assertRequirements(
        CompiledNode $node,
        string $input,
        mixed $source,
        array $arguments,
        int $cursor,
    ): void {
        if (
            $this->requirementsPass(
                $node,
                new Command($input, $source, $arguments),
            )
        ) {
            return;
        }

        throw new CommandRequirementFailed(
            'command requirements were not satisfied',
            $cursor,
        );
    }

    private function requirementsPass(
        CompiledNode $node,
        Command $command,
    ): bool {
        foreach ($node->requirements as $requirement) {
            if (!$requirement($command)) {
                return false;
            }
        }

        return true;
    }

    /** @return list<string> */
    private function rootSuggestions(string $prefix): array
    {
        $suggestions = [];
        foreach (array_keys($this->roots) as $name) {
            if (str_starts_with($name, $prefix)) {
                $suggestions[] = $name;
            }
        }
        sort($suggestions, SORT_STRING);

        return $suggestions;
    }

    private function remove(int $id): bool
    {
        $registration = $this->registrations[$id] ?? null;
        if ($registration === null) {
            return false;
        }

        unset($this->registrations[$id]);
        foreach ($registration['names'] as $name) {
            if (($this->roots[$name] ?? null) === $id) {
                unset($this->roots[$name]);
            }
        }

        return true;
    }

    private static function normalizeInput(string $input): string
    {
        $input = trim($input);
        if (str_starts_with($input, '/')) {
            $input = ltrim(substr($input, 1));
        }

        return $input;
    }
}
