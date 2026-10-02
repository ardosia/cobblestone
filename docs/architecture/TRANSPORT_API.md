# Transport, protocol, session, and connection design

## Status

This document defines the intended architecture and public boundaries for:

- native/transport;
- native/protocol84;
- native/session;
- the session-facing portions of native/extension;
- src/Session;
- future login/identity/session bootstrap concerns.

The design keeps RakNet and protocol-84 wire machinery out of ordinary gameplay APIs.

## Layering

The transport stack is conceptually:

~~~text
UDP / RakNet
    ↓
transport connection
    ↓
protocol-84 frame / packet codec
    ↓
login + bootstrap session
    ↓
gameplay session
    ↓
Player
~~~

Each layer owns one job.

A Player is not a RakNet connection.

A Session is not a Player.

A protocol packet is not gameplay state.

## Native ownership

Rust/native code owns:

- RakNet protocol-8 state;
- socket/network event loops;
- connection admission/backpressure;
- protocol-84 binary encoding/decoding;
- Batch compression/decompression;
- stable native session identity;
- bounded cross-thread queues;
- malformed-wire containment;
- coarse outbound packet/frame work.

PHP owns:

- login policy;
- gameplay semantics;
- player creation;
- world/spawn decisions;
- plugin-visible session/player events;
- kick/disconnect reasons at the semantic level.

## Network backend

The network backend should remain an internal mechanism.

Its contract is:

- one bounded command path into the backend;
- one bounded event path out;
- explicit backpressure;
- stable connection/session identity;
- ordered operations per connection;
- deterministic close semantics.

No plugin API should expose backend command enums, Tokio channels, RakNet state machines, or worker-shard identifiers.

## Connection admission

New transport connections are not Players.

Admission creates a transport/session identity and begins protocol bootstrap.

Connection limits are enforced before expensive gameplay state is allocated.

A connection may be dropped before login without ever producing a Player object.

## Session identity

SessionId is stable for the lifetime of one accepted gameplay-session attempt.

A Session value exposed to PHP may provide stable read-only connection facts such as:

~~~php
$session->id();
$session->remoteAddress();
$session->latency();
$session->state();
~~~

It may provide semantic control:

~~~php
$session->disconnect('Unsupported client state');
~~~

It should not expose mutable RakNet internals.

## Session states

The lifecycle should be explicit enough to reason about bootstrap:

~~~text
Accepted
LoggingIn
Preparing
Playing
Disconnecting
Closed
~~~

The exact internal enum may be more detailed.

Gameplay code should not need to inspect transport substates.

Events should expose only stable phases that matter semantically.

## Player handoff

A Player is created only after the session has enough validated identity/bootstrap state to become gameplay-owned.

The transition should produce a stable association:

~~~text
SessionId -> PlayerId
~~~

After that point, ordinary gameplay should address the Player.

The Session remains useful for low-level connection facts and disconnect ownership.

Player design is specified in GAMEPLAY_API.md.

## Login identity

Login should produce a typed identity value rather than a generic decoded packet array.

Conceptually:

~~~php
final readonly class ClientIdentity
{
    public function name(): string;
    public function clientId(): string|int;
    public function skin(): Skin;
}
~~~

The exact fields are fixed by protocol-84 evidence and should not be invented ahead of source parity.

Authentication policy must be separate from packet parsing.

If the fixed target later supports or requires external identity verification, that verification becomes a policy/provider boundary rather than being fused into the codec.

## Codec

The codec crate owns wire representation.

Decoding should return typed packet/frame structures or compact native projections, not associative PHP arrays for every packet.

Encoding should accept typed semantic packet values from the session/protocol layer.

Wire IDs, endianness, varints, NBT representation, Batch framing, compression, and malformed-input limits belong here.

## Decode once

A payload should not be decoded repeatedly merely to discover its type and then decode it again.

The current Batch path should converge on one outer-frame decode followed by one Batch decode path.

The parser should preserve enough structure from the first pass to route without re-copying/re-parsing the same frame.

## Compression

Batch compression/decompression is CPU work.

It must not monopolize a single current-thread session runtime when load proves material.

The session layer should preserve per-session ordering while allowing bounded CPU-heavy codec work to run on appropriate native workers when measured.

Moving compression work off the session owner must preserve:

- input ordering;
- output ordering where required;
- cancellation on disconnect;
- bounded queued work;
- malformed-input limits.

## Buffer ownership

NativeBuffer/Bytes-style values should be cheap to clone when immutable.

Avoid converting through a full copy merely to cross crate boundaries.

The codec/session boundary should prefer shared immutable byte storage where the involved libraries permit it.

Copies must remain explicit where ownership or library APIs require them.

## Session host

The session host owns active native sessions.

PHP should interact through coarse bounded operations rather than shared-map locking for every small command when a host command queue can carry the same semantics.

The host may internally keep its session map thread-local to the host runtime.

Cross-thread callers send commands such as:

~~~text
Send
Disconnect
SetState
Shutdown
~~~

through one bounded channel.

This design is preferred if benchmarks show the shared mutex path materially contributes overhead.

## PHP polling

PHP should not cross FFI once per available session event when a bounded batch is equivalent.

The target owner-runtime surface is conceptually:

~~~php
foreach ($sessions->pollBatch($budget) as $event) {
    // dispatch semantic session work
}
~~~

The exact representation may be a compact native projection decoded by the PHP session package.

The operation must remain bounded by the caller-supplied budget.

## Session events

The PHP session layer should expose semantic events such as:

~~~text
SessionAccepted
SessionLogin
SessionReady
SessionClosed
~~~

and gameplay/player events after player ownership exists.

Do not emit a generic PacketReceived event for every built-in gameplay packet by default.

Built-in traffic should stay on optimized semantic paths.

## Advanced packet hooks

Advanced protocol access may exist for plugins that deliberately need it.

It should be explicit and typed.

A possible shape is:

~~~php
$plugin->packet(
    MovePlayer::class,
    function (PacketContext $context, MovePlayer $packet): void {
        // advanced protocol hook
    },
);
~~~

This is not the ordinary movement API.

Packet hooks must document:

- owner runtime;
- whether the packet is before or after semantic validation;
- whether modification is allowed;
- ordering relative to built-in handling;
- performance implications.

A raw byte hook should be even more explicit and should not be enabled accidentally.

## Outbound gameplay traffic

Gameplay code should request semantics.

For example:

~~~php
$player->sendMessage('Hello');
$player->teleport($position);
$inventory->setItem($slot, $stack);
~~~

The relevant subsystem translates those operations into protocol updates.

Plugins should not need to construct packet objects for ordinary gameplay.

## Backpressure

Every transport boundary is bounded.

When queues fill, the policy must be explicit.

Possible outcomes include:

- reject a new submission;
- drop nonessential telemetry;
- disconnect a misbehaving or irrecoverably lagging session;
- defer bounded gameplay work.

Unbounded queues are not a recovery mechanism.

## Per-session ordering

Commands affecting one session preserve semantic order.

For example:

~~~text
send A
send B
disconnect
~~~

must not become:

~~~text
disconnect
send B
send A
~~~

merely because CPU work was parallelized.

Ordering across unrelated sessions need not be globally serialized.

## Central network-loop blocking

The backend event loop should avoid awaiting expensive per-connection send work in a way that creates head-of-line blocking across all connections.

The exact fix must be measurement-driven.

Potential mechanisms include:

- per-connection outbound queues;
- shard-local send workers;
- admission plus completion notifications.

The public API must not depend on which mechanism wins.

## Disconnects

Disconnect is semantic and idempotent.

A disconnect reason has:

- an internal machine-readable category;
- an optional client-facing message;
- a log/diagnostic reason.

Do not use arbitrary exception strings as the only disconnect taxonomy.

## Malformed clients

Malformed protocol input must remain isolated to the offending connection/session.

The codec/session boundary should turn malformed input into a typed session failure.

It must not panic the process or leak partially decoded state into gameplay.

## Protocol limits

Every variable-length protocol structure needs configured or fixed-target bounds.

Examples include:

- Batch decompressed size;
- packet count;
- NBT depth/size;
- string size;
- collection counts;
- pending inbound frames.

Limits are applied before allocating attacker-controlled unbounded memory.

## Bootstrap

Session bootstrap owns fixed-target join sequencing.

It coordinates:

- login validation;
- world/session selection;
- initial player state;
- initial chunk view preparation;
- spawn readiness;
- transition to Playing.

Bootstrap may span multiple ticks.

Disk/network/native work must not block the PHP owner runtime while waiting.

The current persistent chunk preparation direction remains valid: native load work runs asynchronously, then PHP advances spawn when the bounded result is ready.

## Chunk streaming

Session/view synchronization consumes world snapshots and change projections.

PHP owns the semantic viewer set and gameplay decisions.

Native code may encode/reuse protocol-84 chunk payloads and coalesce world-change projections.

Per-block network marshaling should not be pushed through PHP when an existing native snapshot/change boundary already represents the operation.

## Protocol cache

Revision-keyed encoded chunk caches remain an internal performance mechanism.

Cache identity must include every revision/input that changes the resulting protocol representation.

A cache hit must never bypass semantic visibility or session state checks.

## Fixed-target rule

Cobblestone targets protocol 84 and RakNet protocol 8 first.

Do not generalize every codec/session interface for hypothetical future versions.

Version-specific code may be isolated where it protects the rest of the architecture, but the fixed target is allowed to simplify public and internal APIs.

## Resource-pack and protocol-gated phases

Features that exist only if protocol-84/source parity proves them should be added as explicit bootstrap phases rather than guessed abstractions.

Examples may include resource-pack negotiation or client metadata phases.

The subsystem map can reserve a place for these without claiming unsupported behavior exists today.

## Native/PHP error boundary

Native panics are contained.

Native session/codec/network errors are translated into stable PHP/session categories.

PHP exceptions must not unwind through Rust FFI.

Rust error strings are diagnostic detail, not the public error type.

## Testing

Transport tests should cover:

- malformed frames;
- Batch limits;
- encode/decode round trips;
- per-session ordering;
- queue saturation;
- disconnect races;
- shutdown with active sessions;
- reconnect/new SessionId behavior;
- bootstrap state transitions;
- fixed 0.15.10 client compatibility.

The production client remains the ultimate compatibility check for wire behavior that unit tests cannot prove.

## Design summary

The transport stack should converge on:

> Native code owns bounded network/protocol machinery. PHP owns login and gameplay semantics. Sessions bridge the two. Players are created only after bootstrap. Built-in gameplay never depends on generic packet events, and ordinary plugins never need RakNet knowledge.
