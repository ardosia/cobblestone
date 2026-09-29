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
├── log/
│   ├── composer.json
│   ├── ConsoleFormatter.php
│   ├── ContextLogger.php
│   └── LoggerFactory.php
├── plugin/
│   ├── composer.json
│   ├── Plugin.php
│   ├── PluginContext.php
│   └── PluginManager.php
├── server/
│   ├── composer.json
│   ├── Server.php
│   ├── ServerRunner.php
│   ├── WorldFactory.php
│   ├── WorldMaintenance.php
│   ├── Event/
│   └── Tick/
├── session/
│   ├── composer.json
│   ├── SessionBootstrap.php
│   ├── SessionGameplay.php
│   ├── InitialChunkView.php
│   ├── ChunkViewPreparation.php
│   ├── Event/
│   └── Native/
├── task/
│   ├── composer.json
│   ├── Scheduler.php
│   ├── DueQueue.php
│   ├── NativeTaskAwait.php
│   └── TickSleep.php
├── world/
│   ├── composer.json
│   ├── World.php
│   ├── Chunk.php
│   ├── MainChunkSource.php
│   ├── NativeWorldStore.php
│   ├── Generator/
│   ├── Mutation/
│   └── Region/
├── world-generation/
│   ├── composer.json
│   ├── FlatGenerator.php
│   ├── FlatLayer.php
│   └── FlatPreset.php
├── world-light/
│   ├── composer.json
│   ├── LightEngine.php
│   ├── LightPropagator.php
│   └── WorldLightAccess.php
└── world-mutation/
    ├── composer.json
    ├── MutationCoordinator.php
    ├── ChunkPatch.php
    └── StagedWorldMutation.php
```

Package roots are their PSR-4 roots; do not add package-local `src/` wrappers. The session package also maps `Cobblestone\Native\Session\` to `session/Native/`, while the server package maps `Cobblestone\Tick\` to `server/Tick/`.

Current dependency direction:

```text
command ──────────────┐
event ────────────────┤
log ──────────────────┼──> plugin ───────────────┐
task ─────────────────┘                          │
                                                │
world ──> session ──────────────────────────────┤
  ├────> world-generation ──────────────────────┤
  ├────> world-light ───────────────────────────┤──> server
  └────> world-mutation ────────────────────────┤
                                                │
command/event/log/plugin/session/task/world ─────┘
```

The root Composer application consumes `modules/php/*` through path repositories and requires only the server composition package directly.

`world/` owns fixed-target coordinates/chunks/world semantics and the native-world facade. `world-generation/`, `world-light/`, and `world-mutation/` own generation, lighting, and staged mutation behavior respectively. Region mapping remains internal execution/ownership plumbing. `log/` owns PSR-3/Monolog application logging. Tick pacing lives under the server composition package, and native session transport lives under the session package. Later gameplay packages such as `player/`, `entity/`, `block/`, and `inventory/` should still be created only when their implementation begins.

## Rust

```text
modules/rust/
├── core/
├── codec/
├── network/
├── session/
├── storage/
└── php-extension/
```

Rust crate names remain stable and each crate keeps normal Cargo-local `src/` directories.

Repository-level module/package layout has no role in Zend function registration. The PHP extension owns native exports independently and exact export names remain regression-tested.
