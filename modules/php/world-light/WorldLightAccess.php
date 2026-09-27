<?php

declare(strict_types=1);

namespace Cobblestone\World\Light;

use Cobblestone\World\BlockPos;
use Cobblestone\World\BlockState;
use Cobblestone\World\Chunk;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\LightAccess;
use Cobblestone\World\LightLayer;
use Cobblestone\World\LightLevel;
use Cobblestone\World\World;
use Cobblestone\World\WorldBounds;

/**
 * Staged LightAccess over one owner-runtime World.
 *
 * Propagation reads its own staged writes. commit() preflights every touched chunk and advances
 * each chunk's light revision once, regardless of how many light cells changed. Reverted staged
 * cells are filtered before revision advancement.
 */
final class WorldLightAccess implements LightAccess
{
    /** @var array<string, array{layer: LightLayer, position: BlockPos, level: LightLevel}> */
    private array $staged = [];

    public function __construct(
        private readonly World $world,
        private readonly BlockLightCatalog $catalog,
    ) {
    }

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

    public function blockState(BlockPos $position): ?BlockState
    {
        if (!$position->isInsideWorld()) {
            return null;
        }

        $chunk = $this->world->chunks()->get($position->chunk());
        if ($chunk === null) {
            return null;
        }

        return $chunk->block($position->localX(), $position->y, $position->localZ());
    }

    public function storedLight(LightLayer $layer, BlockPos $position): ?LightLevel
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
        LightLevel $level,
    ): bool {
        $current = $this->storedLight($layer, $position);
        if ($current === null) {
            return false;
        }
        if ($current->value === $level->value) {
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

        $chunk = $this->world->chunks()->get($position->chunk());
        if ($chunk === null) {
            return null;
        }

        for ($y = $position->y + 1; $y <= WorldBounds::MAX_Y; ++$y) {
            $state = $chunk->block($position->localX(), $y, $position->localZ());
            $properties = $this->catalog->properties($state);
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

        /** @var array<string, array{chunk: Chunk, base: int, entries: list<array{layer: LightLayer, position: BlockPos, level: LightLevel}>}> $groups */
        $groups = [];
        foreach ($this->staged as $entry) {
            $authoritative = $this->authoritativeLight($entry['layer'], $entry['position']);
            if ($authoritative === null) {
                throw new LightPropagationException('light target chunk became unavailable before commit');
            }
            if ($authoritative->value === $entry['level']->value) {
                continue;
            }

            $position = $entry['position']->chunk();
            $key = $position->key();
            if (!isset($groups[$key])) {
                $chunk = $this->world->chunks()->get($position);
                if ($chunk === null) {
                    throw new LightPropagationException('light target chunk became unavailable before commit');
                }

                $groups[$key] = [
                    'chunk' => $chunk,
                    'base' => $chunk->lightRevision()->value,
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
            if ($group['base'] === PHP_INT_MAX) {
                throw new LightPropagationException('chunk light revision space exhausted');
            }
            if ($group['chunk']->lightRevision()->value !== $group['base']) {
                throw new LightPropagationException('chunk light changed while propagation was staged');
            }
        }

        $changed = [];
        foreach ($groups as $group) {
            $chunk = $group['chunk'];
            foreach ($group['entries'] as $entry) {
                $position = $entry['position'];
                if ($entry['layer'] === LightLayer::Sky) {
                    $chunk->setSkyLight(
                        $position->localX(),
                        $position->y,
                        $position->localZ(),
                        $entry['level']->value,
                    );
                } else {
                    $chunk->setBlockLight(
                        $position->localX(),
                        $position->y,
                        $position->localZ(),
                        $entry['level']->value,
                    );
                }
            }

            $chunk->commitLightRevision($group['base'], $group['base'] + 1);
            $changed[] = $chunk->position();
        }

        $this->staged = [];

        return $changed;
    }

    private function authoritativeLight(
        LightLayer $layer,
        BlockPos $position,
    ): ?LightLevel {
        if (!$position->isInsideWorld()) {
            return null;
        }

        $chunk = $this->world->chunks()->get($position->chunk());
        if ($chunk === null) {
            return null;
        }

        $value = $layer === LightLayer::Sky
            ? $chunk->skyLight($position->localX(), $position->y, $position->localZ())
            : $chunk->blockLight($position->localX(), $position->y, $position->localZ());

        return new LightLevel($value);
    }

    private static function key(LightLayer $layer, BlockPos $position): string
    {
        return $layer->name . ':' . $position->x . ':' . $position->y . ':' . $position->z;
    }
}
