# Modules

Cobblestone uses one repository-level product boundary:

```text
modules/
├── php/
└── rust/
```

## PHP

`modules/php/` is a flat workspace of local Composer packages. Package directories and Composer names are lowercase; PHP namespaces/classes remain PascalCase.

```text
modules/php/
├── command/
│   ├── composer.json
│   └── CommandRegistry.php
├── event/
│   ├── composer.json
│   └── EventBus.php
├── native-session/
│   ├── composer.json
│   ├── Runtime.php
│   ├── Connected.php
│   ├── Disconnected.php
│   └── Packet.php
├── plugin/
│   ├── composer.json
│   ├── Plugin.php
│   ├── PluginContext.php
│   └── PluginManager.php
├── server/
│   ├── composer.json
│   ├── Server.php
│   └── Event/
├── session/
│   ├── composer.json
│   ├── JoinFlow.php
│   ├── JoinResult.php
│   └── Event/
├── task/
│   ├── composer.json
│   ├── Scheduler.php
│   ├── NativeTaskAwait.php
│   └── TickSleep.php
└── world/
    ├── composer.json
    ├── World.php
    ├── Chunk.php
    ├── ChunkSection.php
    ├── BlockSource.php
    ├── ChunkSource.php
    ├── MainChunkSource.php
    └── Generator/
```

Package roots are their PSR-4 roots; do not add package-local `src/` wrappers.

Current dependency direction:

```text
command ──────────────┐
event ────────────────┤
task ─────────────────┼──> plugin ──┐
native-session ──> session ─────────┼──> server
native-session ─────────────────────┤
world ──────────────────────────────┘
```

The root Composer application consumes `modules/php/*` through path repositories and requires the server composition package.

`world/` is now active. It owns fixed-target coordinates/chunks/world semantics and Flat generation. Later gameplay packages such as `player/`, `entity/`, `block/`, and `inventory/` are still created only when their implementation begins.

## Rust

```text
modules/rust/
├── core/
├── codec/
├── network/
├── session/
└── php-extension/
```

Rust crate names remain stable and each crate keeps normal Cargo-local `src/` directories.

Repository-level module/package layout has no role in Zend function registration. The PHP extension owns native exports independently and exact export names remain regression-tested.