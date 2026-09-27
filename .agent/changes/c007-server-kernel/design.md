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

The second harness slice therefore acknowledges a bounded radius of two chunks, sends a 5x5 batch of historical layered-format empty chunks, and emits PLAYER_SPAWN. The actual 0.15.10 client crossed that boundary and entered the world, displaying void terrain as expected. After spawn it immediately emitted normal gameplay traffic including MovePlayer (0x10), MobArmorEquipment (0x1c), and PlayerAction (0x20), demonstrating that the client had transitioned out of the loading state.

The synthetic chunks are compatibility probes only; they are not a world implementation and deliberately remain under tests/. Their all-air contents, tiny accepted radius, and extreme zlib compressibility make this join path unrepresentative of production terrain load time.

## Production kernel direction

After the real-client checkpoint, session state gets a stable lifecycle independent of the transport object. The first production slice lives in `native/cobblestone-session`: it assigns non-reused process-local session IDs, hides `Connection`/RakNet types, removes the outer game marker, flattens Batch/compression before delivery to the owner, validates outbound frames before transport submission, closes malformed peers, and propagates the transport's bounded ingress/command backpressure as typed session errors. The session layer still carries protocol packet IDs/bodies internally; those are not plugin/domain APIs.

The PHP kernel owns lifecycle callbacks, events, commands, plugin loading, scheduler state, and Fiber resumption. Native completions are delivered back to the owning PHP runtime through the already-proven completion boundary. Arbitrary Zend calls remain forbidden on native worker/network threads.

The PHP-owner bridge is deliberately split in two. `SessionHost` runs the async session mechanism on a dedicated native thread and exposes only bounded owner events plus nonblocking per-session command queues. `Cobblestone\\Internal\\NativeSessionRuntime` is the single-owner PHP facade: it checks the owning runtime identity, polls typed internal session events on the PHP thread, and queues send/disconnect commands without exposing Tokio, RakNet peer IDs, mutexes, or worker handles. Packet IDs and bodies remain kernel-internal wire data rather than plugin APIs.

Shutdown order is explicit: stop accepting sessions, reject new gameplay work, drain or cancel accepted work according to its contract, disconnect sessions, stop native mechanisms, then tear down PHP-owned state.
