# Public API design principles

## Status

This document defines the cross-cutting design direction for Cobblestone's PHP-facing API.

Subsystem documents may specialize these rules, but they should not casually contradict them. The purpose is not to make every API look identical. The purpose is to make the whole server feel like one library rather than a collection of framework subsystems.

Cobblestone targets PHP 8.5 ZTS and may use modern PHP features directly.

## Governing idea

The public API should follow this rule:

> Objects represent durable state, values represent data, closures represent behavior, handles represent lifetime, and native boundaries operate in coarse batches.

The implementation may contain registries, coordinators, queues, dispatchers, arenas, worker pools, native handles, caches, and protocol state machines.

Ordinary plugin code should not need to understand that architecture.

## Library, not framework

Cobblestone should feel like a library that happens to run a server.

Prefer:

~~~php
$server->on(PlayerJoin::class, $handler);
$server->after(20, $task);
$world->edit($mutation);
$player->teleport($position);
~~~

over:

~~~php
$server->getEventManager()->registerListener(...);
$server->getScheduler()->scheduleDelayedTask(...);
$server->getWorldManager()->getMutationCoordinator()->mutate(...);
~~~

Subsystem objects are valid when they represent real domain state. An Inventory, World, Session, Chunk, Registry, or Logger can be a useful object.

Objects that exist only to expose internal decomposition should stay internal unless advanced users genuinely need them.

## Plain construction

Do not use functional-option syntax merely to imitate another language.

Configuration should be ordinary typed data.

For large configuration surfaces, prefer a readonly configuration value:

~~~php
$config = new ServerConfig(
    bind: '0.0.0.0:19132',
    name: 'Cobblestone',
    maxConnections: 200,
    initialChunkRadius: 3,
    tickRate: 20,
);

$server = Server::create($config);
~~~

For small values, use normal constructors or named arguments.

Avoid:

~~~php
Server::new(
    listen('0.0.0.0:19132'),
    maxConnections(200),
    name('Cobblestone'),
);
~~~

unless the values are genuinely behavioral strategies rather than configuration fields.

## Closures where behavior belongs to an operation

Callbacks are useful when the lifetime or behavior naturally belongs to the call.

Good examples include:

~~~php
$server->on(PlayerJoin::class, function (PlayerJoin $event): void {
    // ...
});

$world->edit(function (WorldEdit $edit): void {
    // ...
});

$world->withChunk($position, function (Chunk $chunk): void {
    // ...
});
~~~

A callback should not be used only to make an API look functional.

## Typed values, not primitive soup

Core domain concepts should have small value types when primitive confusion would be meaningful.

Examples include:

~~~text
BlockPos
ChunkPos
EntityId
RuntimeId
SessionId
WorldId
Permission
GameMode
Difficulty
DamageCause
Tick
Duration
~~~

Do not wrap every integer or string.

Create a value type when it gives at least one of:

- identity safety;
- unit safety;
- validation;
- meaningful domain behavior;
- stable API semantics.

Small immutable value types should normally be readonly.

## Enums for closed sets

Use PHP enums for sets that are closed by the fixed protocol or server semantics.

Examples include game mode, difficulty, event priority, damage causes where the protocol defines a closed set, task state, session state, and internal result categories.

Do not replace extensible registries with enums merely because enums are convenient.

## Interfaces stay small

Interfaces should represent capabilities consumed by a real subsystem.

Avoid giant service interfaces and inheritance trees.

Prefer:

~~~php
interface Generator
{
    public function generate(ChunkDraft $chunk): void;
}
~~~

over one interface containing generation, lighting, saving, ticking, entity management, events, and metadata.

Define an interface because multiple implementations or a test boundary are needed, not because every class is expected to have an interface.

## Inheritance is not the extension mechanism

Normal plugin extensibility should use registration, composition, callbacks, and small interfaces.

Do not require plugins to subclass framework base classes for ordinary events, commands, tasks, entities, items, or worlds.

Inheritance may still be useful where the domain itself is naturally substitutable, but it should not be the default plugin API.

## Explicit lifetime

Anything that registers, pins, schedules, opens, or subscribes should have an explicit lifetime model.

Typical handles include:

~~~text
Subscription
TaskHandle
CommandBinding
ChunkLease
ContainerSession
PluginScope
~~~

A handle may provide deterministic release/cancel behavior and destructor fallback, but correctness must not depend on PHP garbage collection timing.

Plugin-owned handles should normally be attached to the plugin lifetime automatically.

## Scoped operations

Temporary ownership should be represented by a callback when it removes manual cleanup from the common path.

For example:

~~~php
$world->withChunk($position, function (Chunk $chunk): void {
    // the chunk is resident and pinned for this call
});
~~~

Explicit long-lived ownership remains available when needed:

~~~php
$lease = $world->pinChunk($position);

try {
    // ...
} finally {
    $lease->release();
}
~~~

Use the simplest lifetime model that remains deterministic.

## Direct operations

Put an operation on the object that owns the semantic state.

Examples:

~~~php
$player->kick($reason);
$player->teleport($position);
$world->setTime($time);
$inventory->item($slot);
$session->disconnect($reason);
~~~

Avoid routing ordinary domain operations through unrelated managers or global facades.

## Collections

Do not automatically return large PHP arrays from hot or unbounded collections.

For small bounded collections, arrays and iterables are fine.

For large or native-backed collections, prefer one of:

- a bounded query;
- an iterator that does not materialize everything;
- a callback traversal;
- a snapshot object with explicit cost.

Examples:

~~~php
$world->entities($bounds, function (Entity $entity): void {
    // ...
});

foreach ($server->onlinePlayers() as $player) {
    // ...
}
~~~

The implementation should document whether a collection is live, snapshot, or bounded.

## Mutations

Single cheap semantic mutations should be direct:

~~~php
$world->setBlock($position, $state);
$player->setGameMode($mode);
~~~

Related mutations that benefit from one validation/locking/network boundary should support a scoped edit:

~~~php
$world->edit(function (WorldEdit $edit): void {
    $edit->setBlock($a, $stone);
    $edit->setBlock($b, $air);
});
~~~

Do not force every single operation through a transaction object, and do not make every scalar mutation cross PHP/native independently when a batch is the real semantic operation.

## Errors

Public API errors should be typed and meaningful.

Examples:

~~~text
ChunkLoadPending
PlayerNotFound
InvalidCommandDefinition
PermissionDenied
ContainerClosed
StaleHandle
WrongOwner
~~~

Do not expose native magic integers or generic RuntimeException/LogicException for every failure.

Native result codes should be translated once at the extension boundary into stable PHP semantics.

Expected control flow should not be represented by noisy exceptions when a nullable value or explicit result is clearer.

## Reflection

Reflection is acceptable at registration and compilation boundaries.

Examples:

- compiling command handlers;
- validating plugin entrypoints;
- deriving event handler parameter types.

Reflection should not run on every tick, packet, event, or command execution.

Compile metadata once and use compact execution structures afterward.

## Attributes

Attributes may provide optional metadata where they improve static discoverability.

They should not be the only way to register ordinary behavior.

Prefer explicit code:

~~~php
$plugin->on(PlayerJoin::class, $handler);
~~~

over mandatory class scanning.

Attributes are most useful for static declarations that do not need runtime composition.

## Async and Fibers

PHP Fibers are an owner-runtime cooperative mechanism, not a promise that arbitrary PHP runs concurrently.

The public API must not imply parallel execution when work is only cooperatively scheduled.

Native jobs may run on workers, but mutable PHP gameplay state remains owned by its PHP runtime.

Awaiting native work should suspend the Fiber without blocking the server thread.

## No hidden global server

Avoid APIs that depend on an implicit global current server, world, player, or plugin.

Do not make:

~~~php
Server::current();
World::current();
Plugin::current();
~~~

the normal dependency model.

Explicit object references preserve testability, ownership, and future multi-runtime behavior.

## Native boundary

PHP owns gameplay semantics and developer experience.

Rust/native code owns mechanisms, concurrency infrastructure, networking, wire representation, native state, persistence machinery, and measured hot paths.

Do not move a subsystem into Rust because it feels lower level.

Move or batch work only when ownership correctness or measurements justify it.

The native boundary should prefer coarse operations such as:

- submit a bounded job batch;
- take a bounded completion batch;
- apply a world patch;
- snapshot a chunk;
- poll a bounded session-event batch;
- flush a bounded world-change batch.

Avoid per-element FFI crossings in hot loops when one batch represents the same semantic operation.

## Fixed-target semantics

Cobblestone initially targets Minecraft Windows 10 Edition Beta / MCPE 0.15.10, game protocol 84, RakNet protocol 8.

The API should expose game semantics, not protocol packet shapes, unless a user deliberately enters an advanced protocol surface.

A PlayerMove packet is transport input. Player movement is gameplay state.

A FullChunkData packet is wire representation. A Chunk is world state.

Protocol IDs belong in codec/catalog boundaries rather than leaking through ordinary plugin APIs.

## Registration ownership

Events, commands, tasks, recipes, permissions, and other plugin registrations should have an owner.

When plugin enable fails, partial registrations must be rolled back.

When a plugin unloads, its registrations must be released deterministically.

This requirement applies even if the public API hides the individual handles in normal plugin code.

## Performance policy

Ergonomic APIs are not permission to hide unbounded work.

Every hot subsystem should make these costs visible in its internal design:

- PHP object allocation;
- PHP/native crossings;
- locking;
- copying;
- compression;
- collection scans;
- reflection;
- network fan-out;
- snapshot cloning.

Optimize measured boundaries, not imagined ones.

## Naming

Prefer short domain verbs:

~~~text
open
create
close
send
read
write
find
edit
save
load
spawn
remove
kick
teleport
cancel
subscribe
emit
run
stop
~~~

Avoid names that encode internal architecture unless the architecture itself is the public concept.

## Stability

Subsystem implementation may change without changing the semantic API.

Public APIs should be kept small enough that Cobblestone can preserve them while replacing:

- internal queues;
- caches;
- storage layouts;
- worker topology;
- native representations;
- packet codecs;
- synchronization mechanisms.

That separation is the point of the public design.

## Summary

Cobblestone's public style is:

> Plain construction for data, direct methods for domain operations, closures for behavior, typed values for semantics, explicit handles for lifetime, immutable values where possible, and coarse native boundaries underneath.
