# Cobblestone repository instructions

Cobblestone follows the external engineering control plane in `millesant/.gpt` on branch `bleeding`.

Before substantial work:

1. resolve and pin the exact current `millesant/.gpt@bleeding` commit;
2. read `BOOTSTRAP.md`, `CONTROL.toml`, `contracts/context.toml`, `contracts/ci.toml`, and `contracts/requests.toml` at that commit;
3. resolve `ardosia/cobblestone`, intended branch, and exact head before mutation;
4. load `.agent/project.toml` and repository-native instructions/specs;
5. conserve branches, never force-push, check the expected head immediately before ref-moving writes, preserve unrelated work, and verify remote postconditions;
6. keep connector collections, logs, broad diffs, and unknown-size payloads behind the context firewall;
7. use the smallest applicable control-plane skill/workflow and record only validation that actually ran.

## Fixed target

The initial server target is exact and intentionally narrow:

- Minecraft Windows 10 Edition Beta / MCPE 0.15.10
- game protocol 84
- RakNet protocol 8
- controlled PHP 8.5 ZTS runtime

Do not generalize for modern Bedrock unless an accepted change explicitly expands scope.

## Architecture invariants

- PHP owns gameplay semantics and the ordinary plugin/developer experience.
- Rust/native modules own mechanisms, concurrency infrastructure, networking, native representations, and measured hot paths.
- A mutable authoritative game object has exactly one owning PHP runtime at a time.
- Stable native handles identify authoritative objects; a handle is not permission to mutate from the wrong runtime.
- Native worker threads do not invoke arbitrary Zend/PHP APIs.
- RakNet transport is not gameplay, protocol wire packets are not domain APIs, and storage encoding is not domain state.
- Queues crossing concurrency boundaries are bounded and define backpressure, cancellation, shutdown, and stale-message behavior.
- Rust panics must not unwind across the PHP/native boundary.
- Normal plugin APIs must not expose runtimes, regions, mutexes, raw pointers, or threading ceremony.
- Performance claims require measurements.

## Evidence

Use Ardosia and fixed-target artifacts as evidence, not authority. Prefer direct fixed-target evidence for compatibility-sensitive behavior. Do not inspect or reverse the target executable/assets unless the active change actually requires that evidence.

## Validation

Run `python tools/ci.py all`. A test or check counts only if it actually ran against the relevant revision.
