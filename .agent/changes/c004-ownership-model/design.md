# C004 design

## Production ownership record

`cobblestone-core` gains a generic `OwnedArena<T>` layered over the existing generational `Arena`. Each live slot stores exactly three things: the value, its current `RuntimeId` owner, and an `OwnershipEpoch`.

`OwnedHandle<T>` is type-safe identity. It exposes slot index/generation for diagnostics and stable serialization boundaries but does not itself grant read or mutation authority.

`OwnershipEpoch` is a typed monotonic value. New values begin at epoch zero. Every successful owner transfer increments the epoch before the new owner can mutate. If the epoch is exhausted, transfer fails without changing the owner or value.

## Operations

The mechanism provides:

- insert/create with an initial owner;
- ownership metadata lookup for routing decisions;
- owner-and-epoch-gated shared/mutable access;
- explicit transfer from the current owner/epoch to a new owner;
- owner-and-epoch-gated removal/reclamation.

Wrong owner, stale epoch, and stale handle are distinct typed failures. Core does not silently route. Higher gameplay/session layers may route a semantic command after inspecting current ownership.

## Reclamation and stale work

Removal is authorized only by the current owner and current epoch. The underlying `Arena` invalidates the generation before a slot is reused, so all old handles remain stale. Ownership epochs additionally reject delayed commands that still carry a once-valid owner/epoch after a transfer.

The C003 ownership torture remains useful as a stress oracle and should be adapted to exercise the production `OwnedArena` rather than maintaining a second ownership implementation.

## Immutable shared-native values

Immutable values do not participate in mutable ownership transfer. `NativeBuffer` and future immutable snapshots/encoded blobs may be cloned across runtimes because their public API exposes immutable bytes and reference-counted sharing only. A shared immutable value cannot be used as authority to mutate the owning gameplay object from which it originated.

## API boundary

These types are native mechanism surfaces for internal server/runtime integration. Ordinary plugin APIs continue to expose semantic objects and operations, not `RuntimeId`, `OwnershipEpoch`, `OwnedHandle`, locks, threads, or regions.
