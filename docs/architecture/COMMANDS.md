# Command API design

## Status

This document defines the public command API direction for Cobblestone.

The typed-tree foundation is implemented in `src/Command`: literal and argument nodes, variadic immutable composition, aliases, typed handler compilation, requirements, suggestions, plugin-owned bindings, and basic string/integer/boolean/enum argument types.

The design is deliberately Brigadier-like in structure, but it is not a Java Brigadier port. Cobblestone keeps the useful command-tree semantics while using PHP 8.5 language features to remove context-map casts, builder ceremony, listener classes, and framework plumbing.

Semantic gameplay argument types, sender-specific command contexts, permission integration, and generated help/usage remain follow-on work as their owning gameplay subsystems become real.

## Goals

The command system should provide:

- a real recursive command tree with literal and argument nodes;
- arbitrary subcommands and nested arguments;
- executable nodes at any depth;
- typed parsed arguments delivered directly to PHP handlers;
- branch-level requirements such as permissions;
- sender constraints without repeated runtime checks;
- context-aware suggestions and completion;
- aliases without duplicate handlers;
- generated usage and help from the same tree used for parsing;
- registration-time validation for malformed or ambiguous command definitions;
- simple composition with ordinary PHP functions;
- plugin ownership and clean unregister semantics when integrated with plugin lifecycle.

The API should feel like modern PHP, not like Java rewritten with PHP syntax.

## Non-goals

The public API should not require:

- string-only route declarations as the primary model;
- manually reading parsed values from an untyped context map;
- command subclasses for ordinary commands;
- annotation or attribute scanning as the only registration mechanism;
- mutable builder objects that silently change after registration;
- separate parser, completion, usage, permission, and help definitions for the same command;
- internal registry or dispatcher objects to leak into normal plugin code.

## Core model

A command definition is a tree.

Each node is one of two broad kinds:

1. A literal node matches one exact command token.
2. An argument node parses one typed value.

Any node may have children. Any valid node may be executable.

For example:

~~~text
world
└── time *
    ├── set
    │   └── <time:int> *
    └── add
        └── <amount:int> *
~~~

An asterisk marks an executable node.

This representation means there is no special implementation concept for a subcommand, sub-subcommand, or inner argument. They are simply deeper nodes in the same tree.

## Basic declaration

The public surface should keep Brigadier's literal/argument/child/execute vocabulary while making child composition natural in PHP.

~~~php
use function Cobblestone\Command\{integer, literal};

$server->command(
    literal('world')->then(
        literal('time')
            ->executes(
                function (Command $command): void {
                    $command->reply((string) $command->world()->time());
                },
            )
            ->then(
                literal('set')->then(
                    integer('time')->executes(
                        function (Command $command, int $time): void {
                            $command->world()->setTime($time);
                        },
                    ),
                ),

                literal('add')->then(
                    integer('amount')->executes(
                        function (Command $command, int $amount): void {
                            $world = $command->world();
                            $world->setTime($world->time() + $amount);
                        },
                    ),
                ),
            ),
    ),
);
~~~

The important PHP-specific rule is that then() is variadic.

A command with several siblings should read as one tree:

~~~php
literal('time')->then(
    literal('get')->executes(...),
    literal('set')->then(...),
    literal('add')->then(...),
    literal('start')->executes(...),
    literal('stop')->executes(...),
)
~~~

It should not require repeated Java-style then(...)->then(...)->then(...) chains.

## Typed handler injection

Parsed arguments should be delivered directly to handler parameters.

Cobblestone should not require this style:

~~~php
$time = $context->getArgument('time', Integer::class);
~~~

Instead:

~~~php
integer('time')->executes(
    function (Command $command, int $time): void {
        $command->world()->setTime($time);
    },
)
~~~

The argument node declares what it parses. The handler parameter name associates the parsed value with its destination. The handler parameter type validates the produced PHP type.

A nested example:

~~~php
literal('give')->then(
    player('player')->then(
        item('item')->then(
            integer('count', min: 1, max: 64)->executes(
                function (
                    Command $command,
                    Player $player,
                    ItemType $item,
                    int $count,
                ): void {
                    // ...
                },
            ),
        ),
    ),
)
~~~

The command compiler should validate the definition when it is registered.

A definition such as:

~~~php
integer('count')->executes(
    fn (Command $command, string $count) => null,
)
~~~

should fail registration with a focused error explaining that the count argument produces int while the handler expects string.

Normal command execution should not perform stringly typed lookups or repeated reflection.

## Argument types

Cobblestone should provide small semantic argument constructors for common values:

~~~php
word('name')
string('value')
greedyString('message')

integer('count')
floatArg('speed')
boolean('enabled')

player('player')
world('world')
item('item')
position('position')
~~~

The exact final names may change during implementation, but the distinction matters:

- lexical argument nodes parse generic values;
- semantic argument nodes resolve Cobblestone domain values;
- custom argument types can participate in the same tree, parsing, suggestions, and usage machinery.

## Enums

PHP enums should map naturally to command arguments.

~~~php
enum GameMode: string
{
    case Survival = 'survival';
    case Creative = 'creative';
    case Adventure = 'adventure';
}
~~~

A command may then declare:

~~~php
literal('gamemode')->then(
    enumArg('mode', GameMode::class)->executes(
        function (PlayerCommand $command, GameMode $mode): void {
            $command->player()->setGameMode($mode);
        },
    ),
)
~~~

The enum argument can derive valid literals and completion candidates from the enum cases.

The handler receives the enum value directly.

## Optional tails

Optional trailing arguments should normally be represented by executable parent nodes rather than a separate optional-wrapper abstraction.

For:

~~~text
/tp <target>
/tp <target> <destination>
~~~

the tree is:

~~~php
literal('tp')->then(
    player('target')
        ->executes(
            function (PlayerCommand $command, Player $target): void {
                $target->teleport($command->player()->position());
            },
        )
        ->then(
            player('destination')->executes(
                function (
                    Command $command,
                    Player $target,
                    Player $destination,
                ): void {
                    $target->teleport($destination->position());
                },
            ),
        ),
)
~~~

The earlier executable node is the shorter valid command. Its executable child is the longer valid command.

This keeps optional command structure visible in the tree rather than hiding it in parser-specific syntax.

## Greedy arguments

A greedy string consumes the remaining command input.

~~~php
literal('say')->then(
    greedyString('message')->executes(
        function (Command $command, string $message): void {
            $command->server()->broadcast($message);
        },
    ),
)
~~~

Greedy arguments must be terminal unless an argument type explicitly defines a safe continuation model.

The command compiler should reject unreachable children.

## Literal precedence

When a literal and an argument can both match the same token, literals win.

Given:

~~~text
ban
├── list *
└── <player:Player> *
~~~

the input:

~~~text
/ban list
~~~

must select the literal list branch rather than trying to resolve a player named "list".

Argument ambiguity beyond literal precedence should be detected at registration time where possible and reported clearly.

## Suggestions

Suggestions belong to argument/tree definitions so completion and parsing share one structure.

Semantic arguments should provide their ordinary suggestions automatically:

~~~php
player('player')      // online player names
world('world')        // loaded world names
item('item')          // known item identifiers
enumArg('mode', ...)  // enum cases
~~~

A node may override or augment suggestions when needed:

~~~php
word('world')->suggests(
    function (Suggest $suggest) use ($server): iterable {
        return $suggest->matching($server->worlds()->names());
    },
)
~~~

Suggestion providers may depend on the already-parsed command context.

They must remain bounded and must not make command execution depend on completion being available.

## Requirements

Requirements belong to the branch they govern.

~~~php
literal('admin')
    ->requires(Permission::Admin)
    ->then(
        literal('stop')->executes(...),
        literal('reload')->executes(...),

        literal('player')->then(
            literal('ban')
                ->requires(Permission::Ban)
                ->then(
                    player('player')->executes(...),
                ),
        ),
    )
~~~

Children inherit parent requirements.

A requirement can therefore guard an entire subtree without repeating permission checks at every executable leaf.

Requirements should support compact built-in values such as permission enums while still allowing advanced predicates where a static permission value is insufficient.

## Sender constraints

Common sender constraints should be expressible through handler types rather than repeated manual checks.

For example:

~~~php
literal('fly')->executes(
    function (PlayerCommand $command): void {
        $command->player()->toggleFlying();
    },
)
~~~

A PlayerCommand handler means the node requires a player sender.

Likewise:

~~~php
function (ConsoleCommand $command): void
~~~

means console-only, while:

~~~php
function (Command $command): void
~~~

accepts any valid command sender.

The command compiler should derive and validate the sender requirement from the handler type.

Explicit sender predicates can still exist for uncommon cases, but ordinary player/console constraints should not require boilerplate.

## Aliases

Aliases should share the same tree and handler rather than duplicate command definitions.

A compact form is preferred:

~~~php
literal('teleport', aliases: ['tp'])->then(
    player('target')->executes(...),
)
~~~

Aliases are parser metadata for the literal node. They should participate in usage/help/completion without creating separately owned command trees.

If implementation evidence makes a dedicated aliases() method cleaner, that is an acceptable syntax adjustment as long as the underlying model remains one node with multiple accepted literal names.

## Immutable nodes

Command nodes should be immutable values.

Methods such as then(), requires(), suggests(), and executes() return a new node rather than mutating an already-shared definition.

PHP 8.5's clone-with syntax is a good implementation fit for readonly command nodes.

Conceptually:

~~~php
final readonly class Literal
{
    #[\NoDiscard]
    public function then(Node ...$children): self
    {
        return clone($this, [
            'children' => [...$this->children, ...$children],
        ]);
    }
}
~~~

PHP 8.5's #[\NoDiscard] attribute is particularly useful here because dropping the result of an immutable builder operation is almost certainly a bug:

~~~php
$world = literal('world');

$world->then(
    literal('time'),
);
~~~

The intended usage is composition or reassignment:

~~~php
$world = $world->then(
    literal('time'),
);
~~~

or, more commonly:

~~~php
$server->command(
    literal('world')->then(
        literal('time'),
    ),
);
~~~

Cobblestone targets PHP 8.5, so the command API may use these features directly rather than carrying compatibility machinery for older PHP versions.

Relevant PHP 8.5 language features are documented by PHP at:

https://www.php.net/releases/8.5/en.php

## Composition

Large command trees should be split with ordinary PHP functions that return nodes.

~~~php
$server->command(
    literal('region')->then(
        regionCreate(),
        regionDelete(),
        regionFlag(),
        regionMember(),
        regionInfo(),
    ),
);
~~~

For example:

~~~php
function regionFlag(): Node
{
    return literal('flag')->then(
        literal('get')->then(
            region('region')->then(
                regionFlagArg('flag')->executes(...),
            ),
        ),

        literal('set')->then(
            region('region')->then(
                regionFlagArg('flag')->then(
                    boolean('value')->executes(...),
                ),
            ),
        ),
    );
}
~~~

No inheritance hierarchy is required to organize a command tree.

## Registration and compilation

$server->command($root) should register one root command tree.

Registration should compile and validate the immutable definition into an execution representation suitable for repeated dispatch.

Compilation should validate at least:

- root literal validity;
- duplicate sibling literals and aliases;
- conflicting or unreachable branches;
- greedy argument placement;
- handler argument names;
- handler argument PHP types;
- sender-handler compatibility;
- unsupported handler parameters;
- duplicate or contradictory requirements where applicable.

Reflection may be used during registration/compilation.

Normal execution should use cached compiled metadata rather than repeatedly reflecting handlers.

## Execution

Dispatch should traverse the compiled tree token by token.

The general precedence is:

1. matching literal children;
2. typed argument children according to the command compiler's validated ordering;
3. greedy terminal arguments.

Execution should produce structured parse failures rather than generic exceptions.

Errors should retain enough information to generate useful messages such as:

~~~text
Expected integer for <count>, got "lots".
Unknown subcommand "freeze" after "/world time".
Player "Stevee" was not found.
This command can only be used by a player.
You do not have permission to use "/admin stop".
~~~

User-facing formatting remains a higher-level concern; parser errors should stay structured until the presentation boundary.

## Help and usage

Usage strings, command discovery, and help should be projections of the same compiled tree.

Cobblestone should not require a second hand-maintained usage declaration for ordinary commands.

The tree already contains:

- literals and aliases;
- argument names and types;
- executable boundaries;
- requirements;
- sender restrictions;
- optional paths through executable parent nodes.

Help and usage generation should use that information directly.

## Plugin ownership

Command registration must eventually integrate with plugin lifetime ownership.

Registering a tree should return or create a binding owned by the plugin scope.

When plugin enable fails or a plugin unloads, all command roots registered by that scope must be removed atomically with the rest of that plugin's owned resources.

The command API should not depend on plugin authors manually remembering to unregister every route.

The exact scope/binding API is intentionally left to the broader plugin lifecycle design, but the command registry must support deterministic removal.

## Internal implementation direction

The public tree API should not dictate one exact internal representation.

A likely split is:

- public immutable definition nodes in PHP;
- registration-time compiler/validator in the command package;
- compact compiled nodes for dispatch and completion;
- argument resolvers owned by the command package or the semantic package that defines the argument type;
- event/plugin/server packages interacting through focused registration/execution contracts rather than reaching into private registry state.

The current architectural rule remains unchanged:

> PHP owns gameplay semantics and the developer/plugin experience. Rust/native extensions own mechanisms, concurrency infrastructure, networking, native representations, and measured hot paths.

Command parsing should remain in PHP unless measurement proves a native hot path is necessary. A native rewrite is not part of this design.

## Design principles

The command API should optimize for these rules:

- keep the real command tree;
- expose the tree without exposing Java ceremony;
- use PHP types as useful schema;
- let enums be enums;
- let functions compose trees;
- make branches own requirements;
- make executable parent nodes model optional tails;
- make suggestions part of argument semantics;
- validate once at registration;
- avoid stringly typed context lookups during execution;
- generate usage/help/completion from the same source of truth;
- keep internal registry machinery out of ordinary plugin code.

In short:

> Brigadier semantics, PHP-native ergonomics.
