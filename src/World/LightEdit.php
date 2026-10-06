<?php

declare(strict_types=1);

namespace Cobblestone\World;

/** Staged, atomic chunk-local update to sky and block light channels. */
final class LightEdit
{
    private readonly int $baseRevision;

    /** @var array<int, LightLevel> */
    private array $sky = [];

    /** @var array<int, LightLevel> */
    private array $block = [];

    public function __construct(private readonly Chunk $chunk)
    {
        $this->baseRevision = $chunk->lightRevision()->value;
    }

    public function sky(int $x, int $y, int $z): ?LightLevel
    {
        $key = self::key($x, $y, $z);
        if ($key === null) {
            return null;
        }

        return $this->sky[$key] ?? new LightLevel($this->chunk->skyLight($x, $y, $z));
    }

    public function block(int $x, int $y, int $z): ?LightLevel
    {
        $key = self::key($x, $y, $z);
        if ($key === null) {
            return null;
        }

        return $this->block[$key] ?? new LightLevel($this->chunk->blockLight($x, $y, $z));
    }

    public function setSky(int $x, int $y, int $z, LightLevel $level): ?LightLevel
    {
        $key = self::key($x, $y, $z);
        if ($key === null) {
            return null;
        }

        $previous = $this->sky($x, $y, $z);
        if ($previous?->value !== $level->value) {
            $this->sky[$key] = $level;
        }

        return $previous;
    }

    public function setBlock(int $x, int $y, int $z, LightLevel $level): ?LightLevel
    {
        $key = self::key($x, $y, $z);
        if ($key === null) {
            return null;
        }

        $previous = $this->block($x, $y, $z);
        if ($previous?->value !== $level->value) {
            $this->block[$key] = $level;
        }

        return $previous;
    }

    public function commit(): bool
    {
        if ($this->chunk->lightRevision()->value !== $this->baseRevision) {
            throw new \LogicException('chunk light changed while LightEdit was staged');
        }

        $sky = array_filter(
            $this->sky,
            fn(LightLevel $level, int $key): bool => $level->value !== $this->authoritativeSky($key),
            ARRAY_FILTER_USE_BOTH,
        );
        $block = array_filter(
            $this->block,
            fn(LightLevel $level, int $key): bool => $level->value !== $this->authoritativeBlock($key),
            ARRAY_FILTER_USE_BOTH,
        );

        if ($sky === [] && $block === []) {
            return false;
        }
        if ($this->baseRevision === PHP_INT_MAX) {
            throw new \OverflowException('chunk light revision space exhausted');
        }

        ksort($sky);
        ksort($block);
        $next = $this->baseRevision + 1;

        $skyValues = [];
        foreach ($sky as $key => $level) {
            $skyValues[$key] = $level->value;
        }
        $blockValues = [];
        foreach ($block as $key => $level) {
            $blockValues[$key] = $level->value;
        }

        $terrainRevision = $this->chunk->revision();
        $this->chunk->applyPatch(
            $terrainRevision,
            $terrainRevision,
            $this->baseRevision,
            $next,
            [],
            [],
            [],
            $skyValues,
            $blockValues,
        );

        return true;
    }

    private function authoritativeSky(int $key): int
    {
        [$x, $y, $z] = self::decode($key);

        return $this->chunk->skyLight($x, $y, $z);
    }

    private function authoritativeBlock(int $key): int
    {
        [$x, $y, $z] = self::decode($key);

        return $this->chunk->blockLight($x, $y, $z);
    }

    private static function key(int $x, int $y, int $z): ?int
    {
        return ChunkCoordinateKey::blockOrNull($x, $y, $z);
    }

    /** @return array{int, int, int} */
    private static function decode(int $key): array
    {
        return ChunkCoordinateKey::decodeBlock($key);
    }
}
