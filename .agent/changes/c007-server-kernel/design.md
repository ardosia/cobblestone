# C007 design

## Boundary

C007 owns the session/kernel layer above `cobblestone-network` and `cobblestone-codec`. RakNet remains transport-only, protocol-84 structs remain wire-only, and PHP owns gameplay semantics plus the ordinary plugin/developer API.

The production target is one owning PHP runtime first. Multi-runtime gameplay distribution remains C011 and must not leak into the C007 public surface.

## Real-client bootstrap checkpoint

Before building more kernel surface, C007 starts with a test-only real-client harness under `tests/client-bootstrap`. It exists to answer one compatibility question with the actual 0.15.10 executable: how far the current RakNet-8 plus protocol-84 implementation gets before another required packet/mechanism is missing.

The harness is deliberately not a production server CLI. It:

- binds protocol-8 RakNet on UDP 19132 by default;
- advertises the historical fixed-target server-list layout `MCPE;<name>;84;;<online>;<max>`;
- accepts one connected peer;
- recognizes Login directly or nested in Batch;
- reports only Login envelope sizes, not auth/JWT contents;
- sends LoginSuccess, StartGame, SetTime, SetSpawnPosition, SetDifficulty, and survival AdventureSettings using reliable-ordered delivery;
- keeps the connection alive and prints subsequent protocol-84 packet IDs so the next compatibility gap is evidence-driven.

The historical matching source sends LoginSuccess before StartGame and then initial time/spawn/difficulty state. The first real-client run reached protocol-84 Login successfully and then emitted packet 0x3d (RequestChunkRadius) while remaining on Generating Terrain. That observation matches the historical flow: the server acknowledges with ChunkRadiusUpdated, streams FullChunkData, and only then reports PLAYER_SPAWN.

The second harness slice therefore acknowledges a bounded radius of two chunks, sends a 5x5 batch of historical layered-format empty chunks, and emits PLAYER_SPAWN. The synthetic chunks are compatibility probes only; they are not a world implementation and deliberately remain under tests/.

## Production kernel direction

After the real-client checkpoint, session state gets a stable lifecycle independent of the transport object. Session ingress is bounded and decoded through explicit codec limits. Session egress selects RakNet reliability without exposing RakNet types to PHP gameplay APIs.

The PHP kernel owns lifecycle callbacks, events, commands, plugin loading, scheduler state, and Fiber resumption. Native completions are delivered back to the owning PHP runtime through the already-proven completion boundary. Arbitrary Zend calls remain forbidden on native worker/network threads.

Shutdown order is explicit: stop accepting sessions, reject new gameplay work, drain or cancel accepted work according to its contract, disconnect sessions, stop native mechanisms, then tear down PHP-owned state.
