// Adapted for Cobblestone from Ardosia network transport code under Apache-2.0.
// See docs/provenance/ARDOSIA_REUSE.md for exact source revisions and modifications.

#[test]
fn crate_root_does_not_expose_raknet_dependency() {
    let crate_root = include_str!("../src/lib.rs");

    let exposes_raknet = crate_root
        .lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with("pub "))
        .any(|line| line.contains("raknet_rust"));

    assert!(
        !exposes_raknet,
        "crate root must not directly re-export or expose raknet-rust implementation types"
    );
}

#[test]
fn crate_root_keeps_game_and_protocol84_types_out() {
    let crate_root = include_str!("../src/lib.rs");

    for forbidden in [
        "Player",
        "World",
        "Plugin",
        "Protocol84",
        "Packet84",
        "Minecraft",
        "MCPE",
    ] {
        assert!(
            !crate_root.lines().any(|line| {
                let trimmed = line.trim_start();
                trimmed.starts_with("pub ") && trimmed.contains(forbidden)
            }),
            "crate root unexpectedly exposes {forbidden}"
        );
    }
}

#[test]
fn crate_root_contains_only_transport_facade_exports() {
    let crate_root = include_str!("../src/lib.rs");

    for expected in [
        "NetworkConfig",
        "NetworkConfigError",
        "Connection",
        "NetworkError",
        "Reliability",
        "NetworkServer",
    ] {
        assert!(crate_root.contains(expected), "missing {expected}");
    }
}
