<?php

declare(strict_types=1);

namespace Cobblestone\Session;

use Cobblestone\Native\Session\Packet;
use Cobblestone\Native\Session\Runtime;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\ChunkSnapshot;
use Cobblestone\World\NativeChunkLoadStatus;
use Cobblestone\World\World;
use LogicException;
use ValueError;

/** @internal */
final class SessionBootstrap
{
    private const LOGIN_PACKET = 0x01;
    private const REQUEST_CHUNK_RADIUS_PACKET = 0x3d;
    private const WAIT_LOGIN = 0;
    private const WAIT_CHUNK_RADIUS = 1;
    private const WAIT_CHUNK_LOAD = 2;
    private const SPAWNED = 3;
    private const MAX_INITIAL_CHUNK_RADIUS = 3;

    /** @var array<int, int> */
    private array $states = [];

    /**
     * @var array<int, array{
     *   requested: int,
     *   effective: int,
     *   center: ChunkPos,
     *   positions: list<ChunkPos>,
     *   projection: string
     * }>
     */
    private array $pendingSpawns = [];

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
        unset($this->states[$sessionId], $this->pendingSpawns[$sessionId]);
    }

    /**
     * Advances persistent chunk loads without blocking the owner runtime.
     *
     * @return list<array{sessionId: int, result: JoinResult}>
     */
    public function tick(): array
    {
        $completed = [];

        foreach ($this->pendingSpawns as $sessionId => $pending) {
            if (($this->states[$sessionId] ?? null) !== self::WAIT_CHUNK_LOAD) {
                unset($this->pendingSpawns[$sessionId]);
                continue;
            }

            if (!$this->preparePersistentChunks($pending['positions'], $pending['projection'])) {
                continue;
            }

            unset($this->pendingSpawns[$sessionId]);
            $completed[] = [
                'sessionId' => $sessionId,
                'result' => $this->finishSpawn(
                    $sessionId,
                    $pending['requested'],
                    $pending['effective'],
                    $pending['center'],
                    count($pending['positions']),
                ),
            ];
        }

        return $completed;
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
            $center = $this->world->spawn()->chunk();

            return $this->startSpawn(
                $packet->sessionId,
                $requestedRadius,
                $effectiveRadius,
                $center,
            );
        }

        if ($state === self::WAIT_CHUNK_LOAD) {
            $pending = $this->pendingSpawns[$packet->sessionId]
                ?? throw new LogicException("missing pending spawn state for session {$packet->sessionId}");

            if ($packet->packetId === self::REQUEST_CHUNK_RADIUS_PACKET) {
                return JoinResult::chunksLoading(
                    $pending['requested'],
                    $pending['effective'],
                );
            }

            throw new LogicException(
                "initial chunks are still loading for session {$packet->sessionId}",
            );
        }

        return JoinResult::gameplay();
    }

    private function startSpawn(
        int $sessionId,
        int $requestedRadius,
        int $effectiveRadius,
        ChunkPos $center,
    ): JoinResult {
        $nativeStore = $this->world->nativeStore();
        if ($nativeStore === null || !$nativeStore->hasStorage()) {
            $chunkCount = $this->ensureInitialChunks($effectiveRadius, $center);

            return $this->finishSpawn(
                $sessionId,
                $requestedRadius,
                $effectiveRadius,
                $center,
                $chunkCount,
            );
        }

        $positions = $this->initialChunkPositions($effectiveRadius, $center);
        $projection = $nativeStore::encodeStorageLoadBatch($positions);
        if ($this->preparePersistentChunks($positions, $projection)) {
            return $this->finishSpawn(
                $sessionId,
                $requestedRadius,
                $effectiveRadius,
                $center,
                count($positions),
            );
        }

        $this->pendingSpawns[$sessionId] = [
            'requested' => $requestedRadius,
            'effective' => $effectiveRadius,
            'center' => $center,
            'positions' => $positions,
            'projection' => $projection,
        ];
        $this->states[$sessionId] = self::WAIT_CHUNK_LOAD;

        return JoinResult::chunksLoading($requestedRadius, $effectiveRadius);
    }

    /**
     * @param list<ChunkPos> $positions
     */
    private function preparePersistentChunks(array $positions, string $projection): bool
    {
        $nativeStore = $this->world->nativeStore()
            ?? throw new LogicException('persistent chunk preparation requires native world storage');
        if (!$nativeStore->hasStorage()) {
            throw new LogicException('persistent chunk preparation lost its native storage attachment');
        }

        $statuses = $nativeStore->prepareStorageLoadBatch($projection);
        if (strlen($statuses) !== count($positions)) {
            throw new LogicException('persistent chunk preparation returned the wrong status width');
        }

        $resolved = [];
        foreach ($positions as $index => $position) {
            $status = NativeChunkLoadStatus::from(ord($statuses[$index]));
            if (
                $status !== NativeChunkLoadStatus::Resident
                && $status !== NativeChunkLoadStatus::Missing
            ) {
                return false;
            }
            $resolved[] = $status;
        }

        foreach ($positions as $index => $position) {
            if ($resolved[$index] === NativeChunkLoadStatus::Resident) {
                if ($this->world->chunk($position, false) === null) {
                    $this->world->adoptNativeChunk($position);
                }
                continue;
            }

            if ($this->world->chunk($position, true) === null) {
                throw new LogicException(
                    "world failed to generate durably missing initial chunk {$position->x}:{$position->z}",
                );
            }
        }

        return true;
    }

    private function finishSpawn(
        int $sessionId,
        int $requestedRadius,
        int $effectiveRadius,
        ChunkPos $center,
        int $chunkCount,
    ): JoinResult {
        $encodeStarted = hrtime(true);
        $nativeStore = $this->world->nativeStore();
        if ($nativeStore !== null) {
            $encodedBytes = $this->sessions->sendInitialWorldChunks(
                $sessionId,
                $effectiveRadius,
                $nativeStore->handle(),
                $center->x,
                $center->z,
            );
        } else {
            $snapshots = $this->initialChunkSnapshots($effectiveRadius, $center);
            $encodedBytes = $this->sessions->sendInitialChunks(
                $sessionId,
                $effectiveRadius,
                self::nativeProjection($snapshots),
            );
        }
        $encodeNanos = hrtime(true) - $encodeStarted;

        $this->states[$sessionId] = self::SPAWNED;

        return JoinResult::spawned(
            $requestedRadius,
            $effectiveRadius,
            $chunkCount,
            $encodedBytes,
            $encodeNanos,
        );
    }

    /** @return list<ChunkPos> */
    private function initialChunkPositions(int $radius, ChunkPos $center): array
    {
        $positions = [];
        for ($x = $center->x - $radius; $x <= $center->x + $radius; ++$x) {
            for ($z = $center->z - $radius; $z <= $center->z + $radius; ++$z) {
                $positions[] = new ChunkPos($x, $z);
            }
        }

        return $positions;
    }

    private function ensureInitialChunks(int $radius, ChunkPos $center): int
    {
        $count = 0;
        for ($x = $center->x - $radius; $x <= $center->x + $radius; ++$x) {
            for ($z = $center->z - $radius; $z <= $center->z + $radius; ++$z) {
                if ($this->world->chunk(new ChunkPos($x, $z)) === null) {
                    throw new LogicException("world failed to generate initial chunk {$x}:{$z}");
                }
                ++$count;
            }
        }

        return $count;
    }

    /** @return list<ChunkSnapshot> */
    private function initialChunkSnapshots(int $radius, ChunkPos $center): array
    {
        $snapshots = [];

        for ($x = $center->x - $radius; $x <= $center->x + $radius; ++$x) {
            for ($z = $center->z - $radius; $z <= $center->z + $radius; ++$z) {
                $chunk = $this->world->chunk(new ChunkPos($x, $z), false);
                if ($chunk === null) {
                    throw new LogicException("initial chunk {$x}:{$z} disappeared before snapshot");
                }
                $snapshots[] = $chunk->snapshot();
            }
        }

        return $snapshots;
    }

    /**
     * Serializes semantic snapshots into the private PHP/native bulk bridge.
     *
     * This is not a complete protocol-84 packet: block/light planes remain semantic Y/Z/X order,
     * which is already the ORDER_LAYERED terrain order, while biome columns remain semantic IDs.
     * Rust owns validation, biome-word construction, packet framing, compression, and submission.
     *
     * @param list<ChunkSnapshot> $snapshots
     */
    private static function nativeProjection(array $snapshots): string
    {
        $parts = [pack('V', count($snapshots))];

        foreach ($snapshots as $snapshot) {
            $parts[] = self::packInt32Le($snapshot->position->x);
            $parts[] = self::packInt32Le($snapshot->position->z);
            $parts[] = $snapshot->blockIds;
            $parts[] = $snapshot->blockData;
            $parts[] = $snapshot->skyLight;
            $parts[] = $snapshot->blockLight;
            $parts[] = $snapshot->biomes;
            $parts[] = $snapshot->heightMap;

            $extraData = $snapshot->extraData;
            ksort($extraData, SORT_NUMERIC);
            $parts[] = pack('V', count($extraData));
            foreach ($extraData as $key => $value) {
                $parts[] = pack('Vv', $key, $value);
            }
        }

        return implode('', $parts);
    }

    private static function packInt32Le(int $value): string
    {
        if ($value < -0x80000000 || $value > 0x7fffffff) {
            throw new ValueError('protocol-84 chunk coordinate must fit signed 32 bits');
        }

        return pack('V', $value & 0xffffffff);
    }
}
