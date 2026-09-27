<?php

declare(strict_types=1);

namespace Cobblestone\Session;

use Cobblestone\Native\Session\Packet;
use Cobblestone\Native\Session\Runtime;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\ChunkSnapshot;
use Cobblestone\World\World;
use LogicException;
use ValueError;

/** @internal */
final class JoinFlow
{
    private const LOGIN_PACKET = 0x01;
    private const REQUEST_CHUNK_RADIUS_PACKET = 0x3d;
    private const WAIT_LOGIN = 0;
    private const WAIT_CHUNK_RADIUS = 1;
    private const SPAWNED = 2;
    private const MAX_INITIAL_CHUNK_RADIUS = 3;

    /** @var array<int, int> */
    private array $states = [];

    public function __construct(
        private readonly Runtime $sessions,
        private readonly World $world,
        private readonly int $initialChunkRadius = 2,
    ) {
        if ($initialChunkRadius <= 0 || $initialChunkRadius > self::MAX_INITIAL_CHUNK_RADIUS) {
            throw new ValueError('initial chunk radius must be in range 1..3');
        }
    }

    public function connected(int $sessionId): void
    {
        $this->states[$sessionId] = self::WAIT_LOGIN;
    }

    public function disconnected(int $sessionId): void
    {
        unset($this->states[$sessionId]);
    }

    public function handle(Packet $packet): JoinResult
    {
        $state = $this->states[$packet->sessionId] ?? null;
        if ($state === null) {
            throw new LogicException("packet for unknown session {$packet->sessionId}");
        }

        if ($state === self::WAIT_LOGIN) {
            if ($packet->packetId !== self::LOGIN_PACKET) {
                throw new LogicException(
                    "expected Login for session {$packet->sessionId}, got packet {$packet->packetId}",
                );
            }

            $spawn = $this->world->spawn();
            $this->sessions->acceptLogin(
                $packet->sessionId,
                $packet->body,
                $this->world->seed(),
                $this->world->generatorType()->value,
                $spawn->x,
                $spawn->y,
                $spawn->z,
                $this->world->time(),
                $this->world->isTimeStarted(),
                $this->world->name(),
            );
            $this->states[$packet->sessionId] = self::WAIT_CHUNK_RADIUS;
            return JoinResult::loginAccepted();
        }

        if ($state === self::WAIT_CHUNK_RADIUS) {
            if ($packet->packetId !== self::REQUEST_CHUNK_RADIUS_PACKET) {
                throw new LogicException(
                    "expected RequestChunkRadius for session {$packet->sessionId}, got packet {$packet->packetId}",
                );
            }

            $requestedRadius = $this->sessions->requestedChunkRadius($packet->body);
            $effectiveRadius = min($requestedRadius, $this->initialChunkRadius);
            $snapshots = $this->initialChunkSnapshots($effectiveRadius);
            $projection = self::nativeProjection($snapshots);

            $encodeStarted = hrtime(true);
            $encodedBytes = $this->sessions->sendInitialChunks(
                $packet->sessionId,
                $effectiveRadius,
                $projection,
            );
            $encodeNanos = hrtime(true) - $encodeStarted;

            $this->states[$packet->sessionId] = self::SPAWNED;
            return JoinResult::spawned(
                $requestedRadius,
                $effectiveRadius,
                count($snapshots),
                $encodedBytes,
                $encodeNanos,
            );
        }

        return JoinResult::gameplay();
    }

    /** @return list<ChunkSnapshot> */
    private function initialChunkSnapshots(int $radius): array
    {
        $center = $this->world->spawn()->chunk();
        $snapshots = [];

        for ($x = $center->x - $radius; $x <= $center->x + $radius; ++$x) {
            for ($z = $center->z - $radius; $z <= $center->z + $radius; ++$z) {
                $chunk = $this->world->chunk(new ChunkPos($x, $z));
                if ($chunk === null) {
                    throw new LogicException("world failed to generate initial chunk {$x}:{$z}");
                }
                $snapshots[] = $chunk->snapshot();
            }
        }

        return $snapshots;
    }

    /**
     * Serializes semantic snapshots into the private PHP/native bulk bridge.
     *
     * This is not protocol-84 wire data: block/light planes remain semantic Y/Z/X order and
     * biome columns remain IDs. Rust owns all fixed-target wire transposition and biome colors.
     *
     * @param list<ChunkSnapshot> $snapshots
     */
    private static function nativeProjection(array $snapshots): string
    {
        $projection = pack('V', count($snapshots));

        foreach ($snapshots as $snapshot) {
            $projection .= self::packInt32Le($snapshot->position->x);
            $projection .= self::packInt32Le($snapshot->position->z);
            $projection .= $snapshot->blockIds;
            $projection .= $snapshot->blockData;
            $projection .= $snapshot->skyLight;
            $projection .= $snapshot->blockLight;
            $projection .= $snapshot->biomes;
            $projection .= $snapshot->heightMap;

            $extraData = $snapshot->extraData;
            ksort($extraData, SORT_NUMERIC);
            $projection .= pack('V', count($extraData));
            foreach ($extraData as $key => $value) {
                $projection .= pack('Vv', $key, $value);
            }
        }

        return $projection;
    }

    private static function packInt32Le(int $value): string
    {
        if ($value < -0x80000000 || $value > 0x7fffffff) {
            throw new ValueError('protocol-84 chunk coordinate must fit signed 32 bits');
        }

        return pack('V', $value & 0xffffffff);
    }
}
