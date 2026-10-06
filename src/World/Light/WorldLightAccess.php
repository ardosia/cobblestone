<?php

declare(strict_types=1);

namespace Cobblestone\World\Light;

use Cobblestone\World\BlockPos;
use Cobblestone\World\Chunk;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\ChunkSnapshot;
use Cobblestone\World\LightLayer;
use Cobblestone\World\LightLevel;
use Cobblestone\World\World;
use Cobblestone\World\WorldBounds;

/**
 * Staged lighting access over one owner-runtime World.
 *
 * Propagation reads its own staged writes. commit() preflights every touched chunk and advances
 * each chunk's light revision once, regardless of how many light cells changed. Reverted staged
 * cells are filtered before revision advancement.
 */
final class WorldLightAccess
{
    /** @var array<string, array{layer: LightLayer, position: BlockPos, level: int}> */
    private array $staged = [];

    /** @var array<string, ChunkSnapshot> */
    private array $snapshots = [];

    public function __construct(
        private readonly World $world,
        private readonly BlockLightCatalog $catalog,
    ) {}

    public function neighborhoodAvailable(BlockPos $position): bool
    {
        foreach ([
            $position,
            new BlockPos($position->x - 1, $position->y, $position->z),
            new BlockPos($position->x + 1, $position->y, $position->z),
            new BlockPos($position->x, $position->y, $position->z - 1),
            new BlockPos($position->x, $position->y, $position->z + 1),
        ] as $neighbor) {
            if ($this->world->chunks()->get($neighbor->chunk()) === null) {
                return false;
            }
        }

        return true;
    }

    public function blockStateId(BlockPos $position): ?int
    {
        if (!$position->isInsideWorld()) {
            return null;
        }

        return $this->snapshot($position->chunk())?->blockStateId(
            $position->localX(),
            $position->y,
            $position->localZ(),
        );
    }

    public function storedLight(LightLayer $layer, BlockPos $position): ?int
    {
        if (!$position->isInsideWorld()) {
            return null;
        }

        $key = self::key($layer, $position);
        if (isset($this->staged[$key])) {
            return $this->staged[$key]['level'];
        }

        return $this->authoritativeLight($layer, $position);
    }

    public function setStoredLight(
        LightLayer $layer,
        BlockPos $position,
        int $level,
    ): bool {
        if ($level < LightLevel::MIN_VALUE || $level > LightLevel::MAX_VALUE) {
            throw new \ValueError('fixed-target light level must be in range 0..15');
        }

        $current = $this->storedLight($layer, $position);
        if ($current === null) {
            return false;
        }
        if ($current === $level) {
            return true;
        }

        $this->staged[self::key($layer, $position)] = [
            'layer' => $layer,
            'position' => $position,
            'level' => $level,
        ];

        return true;
    }

    public function canSeeSky(BlockPos $position): ?bool
    {
        if (!$position->isInsideWorld()) {
            return null;
        }

        $snapshot = $this->snapshot($position->chunk());
        if ($snapshot === null) {
            return null;
        }

        for ($y = $position->y + 1; $y <= WorldBounds::MAX_Y; ++$y) {
            $stateId = $snapshot->blockStateId($position->localX(), $y, $position->localZ());
            if ($stateId === null) {
                return null;
            }
            $properties = $this->catalog->propertiesForStateId($stateId);
            if ($properties === null) {
                return null;
            }
            if ($properties->lightBlock > 0) {
                return false;
            }
        }

        return true;
    }

    /** @return list<ChunkPos> */
    public function commit(): array
    {
        if ($this->staged === []) {
            return [];
        }

        /** @var array<string, array{chunk: Chunk, snapshot: ChunkSnapshot, entries: list<array{layer: LightLayer, position: BlockPos, level: int}>}> $groups */
        $groups = [];
        foreach ($this->staged as $entry) {
            $authoritative = $this->authoritativeLight($entry['layer'], $entry['position']);
            if ($authoritative === null) {
                throw new LightPropagationException('light target chunk became unavailable before commit');
            }
            if ($authoritative === $entry['level']) {
                continue;
            }

            $position = $entry['position']->chunk();
            $key = $position->key();
            if (!isset($groups[$key])) {
                $chunk = $this->world->chunks()->get($position);
                $snapshot = $this->snapshot($position);
                if ($chunk === null || $snapshot === null) {
                    throw new LightPropagationException('light target chunk became unavailable before commit');
                }

                $groups[$key] = [
                    'chunk' => $chunk,
                    'snapshot' => $snapshot,
                    'entries' => [],
                ];
            }
            $groups[$key]['entries'][] = $entry;
        }

        if ($groups === []) {
            $this->staged = [];

            return [];
        }

        foreach ($groups as $group) {
            $chunk = $group['chunk'];
            $snapshot = $group['snapshot'];
            if ($snapshot->lightRevision === PHP_INT_MAX) {
                throw new LightPropagationException('chunk light revision space exhausted');
            }
            if ($chunk->revision() !== $snapshot->revision) {
                throw new LightPropagationException('chunk terrain changed while propagation was staged');
            }
            if ($chunk->lightRevision()->value !== $snapshot->lightRevision) {
                throw new LightPropagationException('chunk light changed while propagation was staged');
            }
        }

        $changed = [];
        foreach ($groups as $group) {
            $chunk = $group['chunk'];
            $snapshot = $group['snapshot'];
            $skyLight = [];
            $blockLight = [];
            foreach ($group['entries'] as $entry) {
                $position = $entry['position'];
                $key = ($position->y << 8)
                    | ($position->localZ() << 4)
                    | $position->localX();
                if ($entry['layer'] === LightLayer::Sky) {
                    $skyLight[$key] = $entry['level'];
                } else {
                    $blockLight[$key] = $entry['level'];
                }
            }
            ksort($skyLight);
            ksort($blockLight);

            $chunk->applyPatch(
                $snapshot->revision,
                $snapshot->revision,
                $snapshot->lightRevision,
                $snapshot->lightRevision + 1,
                [],
                [],
                [],
                $skyLight,
                $blockLight,
            );
            $changed[] = $chunk->position();
        }

        $this->staged = [];

        return $changed;
    }

    private function authoritativeLight(
        LightLayer $layer,
        BlockPos $position,
    ): ?int {
        if (!$position->isInsideWorld()) {
            return null;
        }

        $snapshot = $this->snapshot($position->chunk());
        if ($snapshot === null) {
            return null;
        }

        return $layer === LightLayer::Sky
            ? $snapshot->skyLightLevel($position->localX(), $position->y, $position->localZ())
            : $snapshot->blockLightLevel($position->localX(), $position->y, $position->localZ());
    }

    private function snapshot(ChunkPos $position): ?ChunkSnapshot
    {
        $key = $position->key();
        if (isset($this->snapshots[$key])) {
            return $this->snapshots[$key];
        }

        $chunk = $this->world->chunks()->get($position);
        if ($chunk === null) {
            return null;
        }

        return $this->snapshots[$key] = $chunk->snapshot();
    }

    private static function key(LightLayer $layer, BlockPos $position): string
    {
        return $layer->name . ':' . $position->x . ':' . $position->y . ':' . $position->z;
    }
}
