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
    public function __construct(private Chunk $chunk)
    {
    }

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

    public function commitPrepared(PreparedTerrainPatch $prepared): TerrainEditResult
    {
        $current = $this->chunk->revision();
        if ($current !== $prepared->baseRevision()) {
            throw new \LogicException(
                'prepared terrain patch must commit against the revision it preflighted',
            );
        }

        if (!$prepared->changed) {
            return new TerrainEditResult(false, $prepared->revision);
        }

        $blocks = $prepared->blocks();
        $biomes = $prepared->biomes();
        ksort($blocks);
        ksort($biomes);

        if ($this->chunk->nativeStore() !== null) {
            $lightRevision = $this->chunk->lightRevision()->value;
            $this->chunk->applyNativePatch(
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

            return new TerrainEditResult(true, $prepared->revision);
        }

        foreach ($blocks as $key => $stateId) {
            [$x, $y, $z] = TerrainPatch::decodeBlockKey($key);
            $this->chunk->setBlockStateId($x, $y, $z, $stateId);
        }
        foreach ($biomes as $key => $biome) {
            [$x, $z] = TerrainPatch::decodeColumnKey($key);
            $this->chunk->setBiome($x, $z, $biome);
        }

        $this->chunk->commitRevision($current, $prepared->revision->value);

        return new TerrainEditResult(true, $prepared->revision);
    }
}
