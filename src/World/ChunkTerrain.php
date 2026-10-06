<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Chunk-local terrain facade matching Ardosia's revision/edit boundary.
 *
 * PHP owns the semantic edit surface. Native-backed chunks commit the prepared replacement as one
 * atomic patch so physical representation and revision advancement stay inside Rust.
 */
final readonly class ChunkTerrain
{
    public function __construct(private Chunk $chunk) {}

    public function revision(): ChunkRevision
    {
        return $this->chunk->terrainRevision();
    }

    public function data(): Chunk
    {
        return $this->chunk;
    }

    public function edit(): TerrainEdit
    {
        return new TerrainEdit($this);
    }

    public function commitPrepared(PreparedTerrainPatch $prepared): bool
    {
        $current = $this->chunk->revision();
        if ($current !== $prepared->baseRevision()) {
            throw new \LogicException(
                'prepared terrain patch must commit against the revision it preflighted',
            );
        }

        if (!$prepared->changed) {
            return false;
        }

        $blocks = $prepared->blocks();
        $biomes = $prepared->biomes();
        ksort($blocks);
        ksort($biomes);

        $lightRevision = $this->chunk->lightRevision()->value;
        $this->chunk->applyPatch(
            $current,
            $prepared->revision->value,
            $lightRevision,
            $lightRevision,
            $blocks,
            $biomes,
            [],
            [],
            [],
        );

        return true;
    }
}
