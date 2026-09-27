<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Chunk-local light API matching Ardosia's semantic ChunkLight surface.
 *
 * Storage remains embedded in Cobblestone's protocol-friendly Chunk sections; this facade keeps
 * the semantic API separate from that representation.
 */
final readonly class ChunkLight
{
    public function __construct(private Chunk $chunk)
    {
    }

    public function revision(): LightRevision
    {
        return $this->chunk->lightRevision();
    }

    public function snapshot(): LightSnapshot
    {
        return $this->chunk->lightSnapshot();
    }

    public function sky(int $x, int $y, int $z): ?LightLevel
    {
        if (
            !WorldBounds::containsLocal($x)
            || !WorldBounds::containsLocal($z)
            || !WorldBounds::containsY($y)
        ) {
            return null;
        }

        return new LightLevel($this->chunk->skyLight($x, $y, $z));
    }

    public function block(int $x, int $y, int $z): ?LightLevel
    {
        if (
            !WorldBounds::containsLocal($x)
            || !WorldBounds::containsLocal($z)
            || !WorldBounds::containsY($y)
        ) {
            return null;
        }

        return new LightLevel($this->chunk->blockLight($x, $y, $z));
    }

    public function edit(): LightEdit
    {
        return new LightEdit($this->chunk);
    }
}
