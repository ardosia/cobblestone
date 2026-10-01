# Gameplay API design

## Status

This document defines the intended direction for gameplay subsystems that are mostly not implemented yet.

It covers:

- players;
- entities;
- movement;
- interactions;
- blocks as behavior;
- items;
- inventories and containers;
- crafting/recipes;
- damage/combat;
- attributes/effects;
- messaging;
- permissions;
- scoreboard/presentation state where fixed-target support exists.

These designs are semantic targets, not claims that the corresponding packages already exist.

Package boundaries should be introduced when implementation begins and a real dependency boundary exists.

## Player

Player is the gameplay identity associated with one successfully bootstrapped gameplay session.

A Player is not a Session and not a protocol packet stream.

Player owns or exposes semantic gameplay state such as:

~~~php
$player->id();
$player->name();
$player->world();
$player->position();
$player->gameMode();
$player->inventory();
~~~

Common operations should be direct:

~~~php
$player->sendMessage('Hello');
$player->teleport($position);
$player->kick('Maintenance');
$player->setGameMode(GameMode::Creative);
~~~

Ordinary code should not use PlayerManager for these operations.

## Player identity

PlayerId is stable for the lifetime/identity model chosen by Cobblestone.

Connection/session reconnect identity must be treated separately from persistent player/profile identity.

Do not use mutable display name as the only persistent key.

The fixed-target login protocol determines which identity facts are trustworthy; persistent identity policy should not be guessed beyond verified source behavior.

## Player lookup

Common bounded lookups should be straightforward:

~~~php
$server->player($nameOrId);
~~~

A missing player returns null or a focused lookup result rather than throwing a generic exception by default.

Ambiguous partial-name matching should not be silently chosen by the core lookup API.

Commands may use a richer PlayerArgument resolver with explicit ambiguity/error semantics.

## Player collection

Online players may be exposed as a bounded iterable/snapshot:

~~~php
foreach ($server->onlinePlayers() as $player) {
    // ...
}
~~~

The contract should specify whether this is a snapshot for the current call/tick.

Do not expose the mutable internal player registry.

## Entity model

Entity represents an authoritative gameplay object with world position and identity.

Do not build a deep subclass hierarchy for every mob/projectile/item variant.

A small public capability interface is preferred.

Conceptually:

~~~php
interface Entity
{
    public function id(): EntityId;
    public function world(): World;
    public function position(): Position;
    public function teleport(Position $position): void;
    public function remove(): void;
}
~~~

Player may implement Entity if doing so preserves clean semantics.

If Player lifecycle diverges enough, it may instead wrap/share an internal entity identity.

The implementation choice should be made from real gameplay requirements, not inheritance purity.

## Entity types

EntityType identifies a fixed-target entity kind.

Known fixed protocol entity types belong to a catalog/registry.

Spawning should be semantic:

~~~php
$entity = $world->spawn(
    EntityType::Zombie,
    $position,
);
~~~

For types with additional required data, use a typed spawn specification rather than an array:

~~~php
$entity = $world->spawn(
    new ItemEntitySpawn(
        position: $position,
        item: $stack,
    ),
);
~~~

Do not require plugins to manually construct AddEntity packets.

## Entity storage

PHP owns gameplay semantics.

Native storage may eventually own measured hot state such as dense position/velocity data if evidence justifies it.

The public Entity API must not depend on whether authoritative fields are PHP objects, native arrays, or handles internally.

No public ECS is introduced unless plugin/gameplay requirements prove it useful.

## Entity queries

Spatial queries should be bounded and avoid materializing huge arrays by default:

~~~php
$world->entities(
    $bounds,
    function (Entity $entity): void {
        // ...
    },
);
~~~

Filtered forms may exist:

~~~php
$world->entities(
    $bounds,
    type: EntityType::Zombie,
    visit: $handler,
);
~~~

The exact syntax may evolve.

The important point is that spatial indexing stays an implementation detail.

## Entity ticking

Not every entity should imply one arbitrary PHP callback every server tick.

Built-in movement/physics timers should use compact subsystem updates.

PHP hooks fire for meaningful semantic events, not every internal arithmetic step.

Plugin-defined periodic behavior uses the task/event systems and must remain bounded.

## Movement

Incoming MovePlayer traffic is input intent/state, not authoritative mutation by itself.

The movement pipeline should conceptually be:

~~~text
decode client input
    ↓
validate session/player state
    ↓
derive requested move
    ↓
apply movement/physics rules
    ↓
emit cancellable/adjustable semantic hook where appropriate
    ↓
commit authoritative position
    ↓
publish viewer updates
~~~

Plugins should observe/modify movement through semantic movement events or player operations, not raw packet mutation by default.

## Position and rotation

Position and Rotation are immutable value types.

Teleport is distinct from ordinary movement.

A teleport updates authoritative state and resets/adjusts protocol synchronization as required by protocol 84.

Do not implement teleport as "pretend the client sent a move packet."

## Physics

Physics should be deterministic owner-runtime gameplay logic.

Collision queries consume world/block state.

Measured collision hot paths may use native acceleration later, but the public API remains semantic.

A plugin should not need to know chunk lock/section layout to test collision.

## Interactions

Player interactions should be represented by semantic operations/events:

~~~text
block break
block place
block activate
use item
interact entity
drop item
pick up item
~~~

Each interaction follows:

1. validate player/session/world state;
2. derive semantic intent;
3. apply permission/game-mode/range rules;
4. emit the appropriate cancellable event;
5. commit world/inventory/entity mutation;
6. publish protocol changes.

Packet arrival alone must not mutate world state before semantic validation.

## Block placement

Placement combines:

- held ItemStack;
- target block/face;
- resulting BlockState;
- inventory consumption;
- collision rules;
- event decision;
- world edit.

These changes should commit coherently.

A cancelled placement must not leave inventory/world disagreement.

## Block breaking

Breaking combines:

- target state;
- player/game mode/tool;
- break rules;
- event decision;
- block removal;
- drops;
- tool durability;
- viewer update.

Drops are semantic ItemStack values, not packet payloads.

## Items

ItemType identifies one fixed-target item kind.

ItemStack is an immutable value:

~~~php
$stack = new ItemStack(
    type: ItemType::Diamond,
    count: 3,
);
~~~

A stack may include fixed-target metadata/damage and NBT where supported.

Avoid mutable Item objects shared across inventories.

Changing count creates/replaces a stack value.

## Empty stacks

Use one canonical representation for an empty slot.

Do not mix null, count zero, AIR item IDs, and multiple sentinel objects unpredictably.

The exact public choice may be null or ItemStack::empty(), but it must be consistent.

## Item behavior

Using/eating/placing an item is gameplay behavior associated with ItemType plus context.

Do not implement one subclass per item unless real behavior complexity proves that model useful.

A behavior table/registry can map fixed-target item types to focused handlers.

## Inventory

Inventory represents slot state with explicit size and ownership.

A player inventory is a real domain object, so this is acceptable:

~~~php
$inventory = $player->inventory();

$stack = $inventory->item($slot);
$inventory->setItem($slot, $stack);
~~~

The inventory object is not a generic manager.

## Inventory edits

Multi-slot operations should be atomic at the semantic boundary:

~~~php
$inventory->edit(
    function (InventoryEdit $edit): void {
        $edit->take($source, 1);
        $edit->put($target, $stack);
    },
);
~~~

An edit validates all affected slots before commit.

Viewer/protocol updates happen after the commit.

This prevents partial client/server inventory state.

## Inventory transactions

Client inventory transactions must be validated against authoritative server slot state.

The server should reject stale/impossible client transactions rather than trusting client-provided final state.

A transaction may include expected revisions/snapshots internally.

Plugins see semantic inventory events and committed edits, not raw transaction packet fields unless using an advanced protocol hook.

## Containers

A container session represents one player viewing/interacting with one inventory-like target.

Conceptually:

~~~php
$container = $player->open($inventory);
$container->close();
~~~

ContainerSession is an explicit lifetime handle.

Closing is idempotent.

Protocol window IDs remain internal.

Opening a new incompatible container closes/replaces the prior one according to fixed-target semantics.

## Inventory events

Useful semantic events may include:

~~~text
InventoryChange
ContainerOpen
ContainerClose
ItemDrop
ItemPickup
Craft
~~~

Do not emit one generic mutable slot packet event for all inventory semantics.

Events should expose enough information for validation/cancellation without leaking protocol window bookkeeping.

## Recipes

Recipe is immutable semantic data.

Recipe registration belongs to plugin lifetime ownership.

A recipe definition should be typed, not an associative array.

Examples may include shaped, shapeless, furnace/smelting recipes if supported by the fixed target.

The exact class set should follow implemented protocol/gameplay behavior.

## Recipe lookup

Recipe matching may use a compiled index.

Do not linearly invoke arbitrary PHP closures for every recipe on every inventory change.

Plugin-defined custom matching may exist but should be an advanced/explicit cost path.

## Crafting

Crafting validates:

- container/grid state;
- matching recipe;
- required quantities/metadata;
- result;
- remaining items;
- event decision.

Input consumption and output insertion commit atomically.

## Damage

Damage is a typed semantic operation.

A possible value is:

~~~php
final readonly class Damage
{
    public function amount(): float;
    public function cause(): DamageCause;
    public function source(): ?Entity;
}
~~~

Applying damage:

~~~php
$entity->damage($damage);
~~~

should flow through gameplay rules/events before health is committed.

## Damage events

Damage hooks may permit cancellation and amount adjustment:

~~~php
$server->on(
    EntityDamage::class,
    function (EntityDamage $event): void {
        $event->setAmount($event->amount() * 0.5);
    },
);
~~~

Only valid mutable decisions are exposed.

Do not expose internal combat pipeline fields as a generic map.

## Health and death

Living entities expose semantic health state where relevant.

Death is a state transition, not merely health <= 0 arithmetic.

It coordinates:

- death event;
- drops;
- player-specific respawn state;
- entity removal;
- viewer updates.

Player death and entity removal may differ and should not be conflated.

## Combat

Combat builds on damage, movement, attributes, items, and interaction validation.

Reach, attack cooldown behavior, armor, knockback, criticals, and other rules must match the fixed target actually being implemented.

Do not copy modern Minecraft combat semantics into 0.15.10 by assumption.

## Attributes

Attributes are typed numeric gameplay state such as health limits or movement speed where fixed-target behavior uses them.

Use a catalog of AttributeType plus value/modifier state.

Do not expose raw protocol attribute arrays as the gameplay API.

## Effects

Status effects are semantic timed values:

~~~php
$player->addEffect(
    new Effect(
        EffectType::Speed,
        duration: 20 * 30,
        amplifier: 1,
    ),
);
~~~

Duration uses ticks or an explicit duration value, not ambiguous milliseconds.

Expiration is scheduled/ticked efficiently.

Effect application/removal drives protocol state after semantic commit.

## Messaging

Messaging should be direct:

~~~php
$player->sendMessage('hello');
$server->broadcast('restart soon');
~~~

A lightweight Text value may be introduced when formatting, translation keys, or structured messages require it.

Do not force every simple string through a component builder.

Protocol-specific text packet details remain underneath.

## Chat

Incoming chat follows semantic validation before broadcast:

~~~text
decode text packet
validate player/session
apply rate/length rules
emit chat event
publish message
~~~

Plugins should use a PlayerChat event, not mutate a Text packet.

Chat spam/rate policy belongs to gameplay/server policy.

## Permissions

Permissions are semantic identifiers used by commands and gameplay checks.

A Permission value may wrap a normalized string:

~~~php
$player->can(new Permission('server.kick'));
~~~

Core permissions may be enums/constants.

Plugins may define their own permission names.

Do not require inheritance from a permission-subject base class.

## Permission resolution

Permission resolution combines:

- built-in defaults;
- operator/admin state if supported;
- persistent grants/denies;
- plugin/session-scoped attachments where needed.

Resolution should be compiled/cached per subject and invalidated on changes.

Do not walk arbitrary wildcard strings on every permission check.

If wildcard semantics are supported, they must be precisely defined.

## Command permissions

COMMANDS.md requirements integrate with the same permission resolver.

~~~php
literal('kick')
    ->requires(Permission::Kick)
~~~

The command system should not implement a second permission model.

## Permission ownership

Plugin-defined temporary grants/requirements attach to plugin lifetime where relevant.

Removing a plugin must not leave stale permission callbacks.

## Game mode

GameMode is an enum.

Changing game mode is semantic:

~~~php
$player->setGameMode(GameMode::Creative);
~~~

It coordinates protocol state and gameplay rules.

Do not scatter mode checks as raw integers across plugins.

## Scoreboard and presentation state

Scoreboards, titles, action bars, boss bars, or similar presentation systems should only be introduced if supported by the fixed target.

When implemented, expose semantic objects/operations rather than packet constructors.

Persistent gameplay state and per-viewer presentation state should remain distinct.

## Plugin-defined gameplay behavior

Plugins extend gameplay through:

- events;
- commands;
- tasks;
- recipe/permission registrations;
- typed behavior registries where a subsystem explicitly supports them.

Plugins should not replace core objects by subclassing Server, World, Player, or native handle wrappers.

## Persistence

Persistent player/entity/inventory state is specified in DATA_API.md.

Gameplay objects should not write files directly.

Persistence snapshots should be immutable/owned values suitable for background I/O.

## Native acceleration

Potential native hot paths include:

- broad-phase entity spatial queries;
- collision scans;
- large inventory/recipe matching tables;
- dense attribute/effect ticking;
- bulk protocol projection.

These are candidates only after measurement.

Gameplay semantics stay in PHP unless there is a demonstrated reason to move a mechanism.

## Testing

Future gameplay tests should cover:

- session-to-player lifecycle;
- player lookup/identity;
- movement validation;
- teleport;
- block break/place atomicity;
- item stack invariants;
- inventory transaction rejection;
- container lifetime;
- recipe matching/craft commit;
- damage/death;
- effect expiration;
- permission resolution/cache invalidation;
- disconnect during gameplay operations;
- world/entity ownership transitions.

Fixed-target behavior must be checked against pinned source/protocol evidence rather than modern Minecraft assumptions.

## Design summary

The gameplay layer should converge on:

> Player and Entity are semantic identities, not packets. ItemStack is immutable data. Inventory edits and interactions commit atomically. Movement, combat, blocks, crafting, effects, and permissions are typed gameplay operations. Plugins extend behavior through events/registrations instead of framework inheritance or raw protocol mutation.
