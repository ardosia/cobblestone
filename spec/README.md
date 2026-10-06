# Fixed-target specification

`spec/` is the human-authored source of truth for immutable MCPE 0.15.10 facts that must agree across Cobblestone languages or subsystems. Generated Rust/PHP sources are committed for normal builds and IDE/static-analysis use.

Put a fact here only when multiple consumers must agree on the same fixed-target identity or layout. Target identity/protocol numbers, chunk dimensions, public block identities, the executable-only End Portal identity, and registered biome IDs/default colors belong here.

Do not move algorithm-local compatibility constants, structure coordinates, RNG constants, terrain thresholds, server policy, queue sizes, worker limits, or other Cobblestone configuration here. Those values stay beside the code that gives them meaning.

Run `cargo xtask generate` after editing the specification. `cargo xtask generate --check` verifies that committed generated files are current without modifying the working tree.
