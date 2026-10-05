<?php

declare(strict_types=1);

namespace Cobblestone\Session\Internal;

use Cobblestone\World\ChunkLoadPending;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\ChunkSnapshot;
use Cobblestone\Native\World\LoadStatus;
use Cobblestone\World\World;
use LogicException;
use ValueError;

/** @internal */
final class InitialChunkView
{
    public function __construct(private readonly World $world)
    {
    }

    /** @return list<ChunkPos> */
    public function positions(int $radius, ChunkPos $center): array
    {
        $positions = [];
        for ($x = $center->x - $radius; $x <= $center->x + $radius; ++$x) {
            for ($z = $center->z - $radius; $z <= $center->z + $radius; ++$z) {
                $positions[] = new ChunkPos($x, $z);
            }
        }

        return $positions;
    }

    public function ensure(int $radius, ChunkPos $center): int
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

    /**
     * @param list<ChunkPos> $positions
     */
    public function preparePersistent(array $positions, string $projection): bool
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

        foreach ($positions as $index => $position) {
            $status = LoadStatus::from(ord($statuses[$index]));
            if ($status !== LoadStatus::Resident && $status !== LoadStatus::Missing) {
                return false;
            }
        }

        // A resident chunk can still be a generated-only neighbor of a populated center.
        // Request each center through the generator before projecting it to the client.
        foreach ($positions as $position) {
            try {
                $chunk = $this->world->chunk($position, true);
            } catch (ChunkLoadPending) {
                return false;
            }

            if ($chunk === null) {
                throw new LogicException(
                    "world failed to populate initial chunk {$position->x}:{$position->z}",
                );
            }
        }

        return true;
    }

    /** @return list<ChunkSnapshot> */
    public function snapshots(int $radius, ChunkPos $center): array
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
     * @param list<ChunkSnapshot> $snapshots
     */
    public function projection(array $snapshots): string
    {
        $parts = [pack('V', count($snapshots))];

        foreach ($snapshots as $snapshot) {
            $parts[] = self::packInt32Le($snapshot->position->x);
            $parts[] = self::packInt32Le($snapshot->position->z);
            $parts[] = $snapshot->blockIds;
            $parts[] = $snapshot->blockData;
            $parts[] = $snapshot->skyLight;
            $parts[] = $snapshot->blockLight;
            $parts[] = $snapshot->biomeWords;
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
