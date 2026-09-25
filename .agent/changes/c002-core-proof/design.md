# C002 design

## Primitive crate

`cobblestone-core` begins as an independently testable safe Rust crate. `RuntimeId` is explicit and nonzero. `Handle<T>` contains stable slot identity plus a nonzero generation; stale generations fail lookup. Removing an object increments the generation before reuse. If the generation cannot be incremented, the slot is retired permanently instead of wrapping.

`NativeBuffer` owns immutable bytes through reference-counted native storage. Constructing from PHP/native mutable input performs an explicit copy or ownership transfer chosen by the adapter; cloning the Rust value is cheap and does not duplicate bytes.

## PHP/Zend adapter

The adapter targets a controlled PHP 8.5 ZTS build. The mechanism crate stays independent of Zend. Entry points validate thread/runtime affinity, convert invalid/stale handles into safe PHP errors, and contain Rust panics before they can cross FFI.

Diagnostic proof APIs may expose runtime identity and primitive operations under an internal namespace, but runtime IDs/handles are not ordinary plugin concepts.

## Workers and completions

A bounded native pool accepts only worker-safe owned/immutable inputs. Worker threads never invoke arbitrary Zend APIs. Completion records target an owning runtime queue; that runtime drains completions and resumes Fibers/dispatches PHP work. Saturation has an explicit backpressure result. Cancellation and shutdown are explicit states, not dropped-message side effects.

## ABI

Sibling native extensions must not reach through private Rust layouts. If cross-extension calls are needed, C002 introduces only the smallest versioned ABI surface required by a real sibling integration. No speculative broad ABI is created.

## Measurement

Benchmarks are added with the mechanism they measure. Results record build/profile/target and exact revision. No throughput/player-count claims are accepted from microbenchmarks alone.
