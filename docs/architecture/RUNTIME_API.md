# Runtime, lifecycle, events, tasks, and plugins

## Status

This document defines the intended PHP-facing design for the runtime-facing subsystems that currently live primarily in:

- src/Server;
- src/Event;
- src/Task;
- src/Plugin;
- src/Log;
- the runtime/worker portions of modules/rust/core and modules/rust/php-extension.

It also defines the future lifecycle model these packages should converge toward.

Command design is specified separately in COMMANDS.md.

## Server lifecycle

Constructing a Server should not start externally observable lifecycle work.

The important distinction is:

1. create/configure the server;
2. register plugins/listeners/commands;
3. start the lifecycle;
4. run ticks until stopped;
5. stop and release owned resources.

A constructor or create() method should not emit ServerStarted before callers have any chance to observe it.

A target shape is:

~~~php
$config = new ServerConfig(
    bind: '0.0.0.0:19132',
    name: 'Cobblestone',
    maxPlayers: 200,
    worldPath: 'worlds/world',
);

$server = Server::create($config);

$server->on(ServerStarted::class, function (ServerStarted $event): void {
    // ...
});

$server->run();
~~~

The exact ServerConfig fields are allowed to evolve.

The lifecycle ordering is not.

## Lifecycle phases

Server lifecycle should be explicit:

~~~text
Created
Starting
Running
Stopping
Stopped
~~~

Starting prepares externally visible runtime resources.

Running owns normal tick/session/gameplay work.

Stopping rejects new lifecycle work where required, cancels owned tasks, drains bounded work according to subsystem guarantees, persists required state, disconnects sessions, unloads plugins, and releases native resources.

Stopped is terminal for that Server instance.

Lifecycle transitions must be idempotent where shutdown paths can race through signal handlers, exceptions, or explicit stop requests.

## Stop requests

Stopping the server is a semantic request, not an immediate process exit.

~~~php
$server->stop();
~~~

A reason may be supplied:

~~~php
$server->stop('Maintenance');
~~~

The server runner remains responsible for reaching a safe stop boundary.

Subsystems should not call exit().

Fatal process-level termination is reserved for failures where safe server ownership cannot continue.

## Tick ownership

One server/runtime owner controls one semantic tick loop.

The owner runtime is responsible for:

- due PHP tasks;
- bounded session events;
- gameplay updates;
- world maintenance;
- bounded persistence/completion work;
- outbound semantic updates;
- lifecycle transitions.

Subsystems should expose bounded work to the tick owner rather than each owning hidden unbounded loops.

Native network threads and worker pools may run independently, but they communicate with the owner through bounded queues and immutable/owned values.

## Server API surface

Normal plugin code should not need manager traversal.

Prefer direct server operations:

~~~php
$server->on(PlayerJoin::class, $handler);
$server->command($tree);
$server->after(20, $task);
$server->every(20, $task);

foreach ($server->onlinePlayers() as $player) {
    // ...
}
~~~

Internally these may delegate to EventBus, CommandRegistry, Scheduler, and player/session registries.

Those implementation objects do not need to become the ordinary public path.

Advanced access may exist when a subsystem object is itself useful, but direct operations remain the default.

## Events

Events are typed semantic objects.

Registration:

~~~php
$subscription = $server->on(
    PlayerJoin::class,
    function (PlayerJoin $event): void {
        // ...
    },
);
~~~

The handler parameter should be validated at registration.

Event dispatch should cache the resolved listener chain for the concrete event class rather than scanning every registered event type on every dispatch.

Registering or removing a listener invalidates only the affected resolution cache.

## Event inheritance

Listeners may subscribe to a base event/interface when the hierarchy carries real semantics.

For example:

~~~php
$server->on(PlayerEvent::class, $handler);
$server->on(PlayerJoin::class, $joinHandler);
~~~

Dispatch of PlayerJoin may call both.

The resolved order should be deterministic and compiled from the concrete type hierarchy.

Arbitrary interface scanning on every emit is not acceptable in a hot path.

## Event priorities

When ordering is needed, use a small closed enum:

~~~php
enum EventPriority: int
{
    case First = 0;
    case Early = 25;
    case Normal = 50;
    case Late = 75;
    case Last = 100;
}
~~~

The exact names/values may change.

A listener registration may supply the priority:

~~~php
$server->on(
    BlockBreak::class,
    $handler,
    priority: EventPriority::Late,
);
~~~

Equal-priority order should remain registration-stable unless a subsystem has a stronger deterministic rule.

## Cancellation

Only events whose semantics support cancellation should be cancellable.

Do not give every event an isCancelled flag.

A cancellable event may expose:

~~~php
$event->cancel();
$event->cancelled();
~~~

or implement a focused CancellableEvent contract.

Cancellation does not mean propagation automatically stops.

The event type defines whether cancelled events continue to lower-priority observers.

Observation-only events remain immutable where practical.

## Event results

Do not use mutable event objects as a generic request/response bus.

When a subsystem needs a decision, use a semantic event/result model.

Examples include:

- cancellation;
- replacement damage amount;
- chat/message transformation;
- target position adjustment where supported.

The allowed mutation should be explicit on that event type rather than a generic data bag.

## Subscription lifetime

on() returns a Subscription handle.

~~~php
$subscription = $server->on(...);
$subscription->cancel();
~~~

Plugin-owned subscriptions should also attach to the plugin lifetime automatically.

Manual cancellation is still useful for dynamically scoped listeners.

Cancellation is idempotent.

## One-shot listeners

A one-shot helper may exist:

~~~php
$server->once(PlayerJoin::class, $handler);
~~~

It is equivalent to a subscription that cancels after its first successful dispatch.

It should not require special event internals.

## Tasks

The task API should distinguish:

- delayed callbacks;
- repeating callbacks;
- cooperative Fiber tasks;
- native jobs/completions.

These have different lifetime and cost characteristics even if one Scheduler implements them internally.

## Delayed and repeating work

Simple tick scheduling should remain simple:

~~~php
$handle = $server->after(
    20,
    function (): void {
        // once after 20 ticks
    },
);

$heartbeat = $server->every(
    20,
    function (): void {
        // once per second at 20 TPS
    },
);
~~~

Scheduling returns TaskHandle, not an integer ID.

~~~php
$heartbeat->cancel();
~~~

TaskHandle should expose only stable lifecycle information needed by callers.

It should not expose heap indexes, native worker IDs, or scheduler internals.

## Cooperative Fiber tasks

Longer stateful cooperative work may use a task context:

~~~php
$handle = $server->task(
    function (Task $task): void {
        while (!$task->cancelled()) {
            // owner-runtime PHP work
            $task->sleep(20);
        }
    },
);
~~~

Starting a task should enqueue it for scheduler execution.

The call to task() should not immediately start the Fiber inline and execute arbitrary user code before returning the handle.

This avoids surprising reentrancy and gives task startup one scheduler-defined boundary.

## Sleeping

Tick sleep is cooperative:

~~~php
$task->sleep(5);
~~~

Sleeping a Fiber must remove it from runnable work without scanning every dormant Fiber on every tick.

The existing due-time heap direction is the correct mechanism.

No wall-clock sleep() or usleep() belongs in owner-runtime gameplay code.

## Native await

A Fiber may await a bounded native job:

~~~php
$result = $task->await($job);
~~~

The public meaning is:

- suspend this Fiber;
- do not block the server thread;
- resume on the owner runtime when the completion is available;
- propagate cancellation/error using stable PHP semantics.

The implementation should batch ready-completion retrieval rather than make one FFI readiness call per waiting task per tick when the waiter count becomes material.

## Cancellation

Task cancellation is cooperative for running PHP behavior and explicit for queued/native jobs where supported.

Cancellation guarantees must be documented.

Cancelling does not imply that arbitrary PHP code is asynchronously interrupted.

A running callback reaches a defined safe point before observing cancellation.

Native jobs receive cancellation tokens where useful.

## Task failure

A task exception should not tear down the scheduler data structures.

The scheduler reports the failure through the runtime's error/logging policy.

Repeating task behavior on exception must be deterministic.

The default should be to stop that repeating task after an uncaught exception rather than fail every future tick forever.

## Plugins

A plugin is a unit of:

- code;
- identity;
- registration ownership;
- configuration/data ownership;
- lifecycle.

A plugin should not be a bag of global static hooks.

## Plugin entrypoint

Cobblestone should support a closure-first entrypoint for small plugins:

~~~php
return static function (PluginScope $plugin): void {
    $plugin->on(PlayerJoin::class, function (PlayerJoin $event): void {
        // ...
    });

    $plugin->command(
        literal('hello')->executes(...),
    );

    $plugin->every(20, function (): void {
        // ...
    });
};
~~~

A class-based entrypoint may also exist for larger plugins:

~~~php
final class ExamplePlugin implements Plugin
{
    public function enable(PluginScope $plugin): void
    {
        // same registration surface
    }
}
~~~

Both forms compile down to the same owned plugin scope.

Class entrypoints must not get extra powers merely because they are classes.

## Plugin scope

PluginScope represents ownership/lifetime, not a service container.

It may expose plugin-owned forms of the ordinary registration API:

~~~php
$plugin->on(...);
$plugin->command(...);
$plugin->after(...);
$plugin->every(...);
$plugin->task(...);
$plugin->cleanup(...);
~~~

Resources registered through the scope are released when the scope closes.

The scope may also provide explicitly scoped resources such as plugin storage/configuration when those designs are implemented.

## Enable rollback

Plugin enable is transactional with respect to plugin-owned registrations.

If enable throws after registering an event, command, and task, those registrations must be removed before the failure escapes.

The PluginManager must not leave partially enabled behavior in global registries.

This is a correctness requirement, not only API polish.

## Plugin unload

Unload closes the scope.

Owned resources are released in deterministic reverse-registration order where ordering matters.

Typical cleanup includes:

- command bindings;
- event subscriptions;
- tasks;
- open plugin-owned container/session hooks;
- registered recipes/permissions where applicable;
- explicit cleanup callbacks.

A destructor is not the unload mechanism.

## Cleanup callbacks

A plugin may register cleanup:

~~~php
$plugin->cleanup(function (): void {
    // release external resource
});
~~~

Cleanup is LIFO.

Cleanup should be used for resources Cobblestone does not already own automatically.

## Plugin dependencies

Plugin dependencies belong in static plugin metadata, not runtime manager lookup code.

Required dependencies determine load order and block enable when unavailable.

Optional dependencies only affect behavior the plugin explicitly checks.

Circular required dependencies are rejected before enable begins.

## Plugin discovery

Discovery/loading and plugin runtime ownership are separate concerns.

Composer/package metadata may identify plugin entrypoints.

The runtime should not require recursive reflection scanning of arbitrary classes to find behavior.

## Logging

Cobblestone continues to use PSR-3-compatible logging.

A plugin may receive or derive a plugin-scoped LoggerInterface.

Logging should preserve structured context internally even when the console renderer is human-oriented.

Avoid formatting large strings in hot paths when the log level is disabled.

## Runtime failures

The owner runtime should distinguish:

- recoverable plugin/task failure;
- malformed client/session input;
- native job failure;
- persistence failure;
- invariant violation that makes continued ownership unsafe.

Not every exception should stop the server.

Not every exception should be swallowed and logged either.

Each subsystem document defines the appropriate boundary.

## Core worker model

Rust core workers remain mechanism-only.

Workers:

- receive immutable or owned inputs;
- never invoke arbitrary Zend APIs;
- use bounded submission/completion queues;
- support cancellation where meaningful;
- return completions to the owning PHP runtime;
- contain panics at the native boundary.

Worker pools are not exposed to plugins as general thread pools.

Plugins express semantic work through higher-level APIs.

## Runtime identity

Runtime identity and generational native handles remain internal ownership mechanisms.

Public gameplay objects may contain native-backed identities, but plugins should not need RuntimeId to perform normal gameplay work.

Wrong-owner behavior routes or rejects according to the subsystem contract.

It never becomes arbitrary cross-thread Zend object access.

## Design summary

The runtime layer should converge on this shape:

> Server owns lifecycle. Event subscriptions, tasks, commands, and plugin registrations have explicit owners. Plugins register behavior through one owned scope. Fibers are cooperative owner-runtime tasks. Native workers are bounded mechanisms, not a userland threading API.
