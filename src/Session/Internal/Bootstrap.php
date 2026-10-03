<?php

declare(strict_types=1);

namespace Cobblestone\Session\Internal;

use Cobblestone\Native\Session\Packet;
use Cobblestone\Native\Session as NativeSession;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\World;
use LogicException;
use ValueError;

/** @internal */
final class Bootstrap
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
    private readonly InitialChunkView $initialChunks;

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
        private readonly NativeSession $sessions,
        private readonly World $world,
        private readonly int $initialChunkRadius = 2,
    ) {
        if ($initialChunkRadius <= 0 || $initialChunkRadius > self::MAX_INITIAL_CHUNK_RADIUS) {
            throw new ValueError('initial chunk radius must be in range 1..3');
        }

        $this->initialChunks = new InitialChunkView($this->world);
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
     * @return list<array{sessionId: int, update: BootstrapUpdate}>
     */
    public function tick(): array
    {
        $completed = [];

        foreach ($this->pendingSpawns as $sessionId => $pending) {
            if (($this->states[$sessionId] ?? null) !== self::WAIT_CHUNK_LOAD) {
                unset($this->pendingSpawns[$sessionId]);
                continue;
            }

            if (!$this->initialChunks->preparePersistent($pending['positions'], $pending['projection'])) {
                continue;
            }

            unset($this->pendingSpawns[$sessionId]);
            $completed[] = [
                'sessionId' => $sessionId,
                'update' => $this->finishSpawn(
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

    public function handle(Packet $packet): BootstrapUpdate
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
            return BootstrapUpdate::loginAccepted();
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
                return BootstrapUpdate::chunksLoading(
                    $pending['requested'],
                    $pending['effective'],
                );
            }

            throw new LogicException(
                "initial chunks are still loading for session {$packet->sessionId}",
            );
        }

        return BootstrapUpdate::gameplay();
    }

    private function startSpawn(
        int $sessionId,
        int $requestedRadius,
        int $effectiveRadius,
        ChunkPos $center,
    ): BootstrapUpdate {
        $nativeStore = $this->world->nativeStore();
        if ($nativeStore === null || !$nativeStore->hasStorage()) {
            $chunkCount = $this->initialChunks->ensure($effectiveRadius, $center);

            return $this->finishSpawn(
                $sessionId,
                $requestedRadius,
                $effectiveRadius,
                $center,
                $chunkCount,
            );
        }

        $positions = $this->initialChunks->positions($effectiveRadius, $center);
        $projection = $nativeStore::encodeStorageLoadBatch($positions);
        if ($this->initialChunks->preparePersistent($positions, $projection)) {
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

        return BootstrapUpdate::chunksLoading($requestedRadius, $effectiveRadius);
    }

    private function finishSpawn(
        int $sessionId,
        int $requestedRadius,
        int $effectiveRadius,
        ChunkPos $center,
        int $chunkCount,
    ): BootstrapUpdate {
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
            $snapshots = $this->initialChunks->snapshots($effectiveRadius, $center);
            $encodedBytes = $this->sessions->sendInitialChunks(
                $sessionId,
                $effectiveRadius,
                $this->initialChunks->projection($snapshots),
            );
        }
        $encodeNanos = hrtime(true) - $encodeStarted;

        $this->states[$sessionId] = self::SPAWNED;

        return BootstrapUpdate::spawned(
            $requestedRadius,
            $effectiveRadius,
            $chunkCount,
            $encodedBytes,
            $encodeNanos,
        );
    }

}
