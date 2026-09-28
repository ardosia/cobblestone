<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Encodes and submits one atomic native chunk patch.
 *
 * @internal
 */
final class NativeChunkPatch
{
    private function __construct()
    {
    }

    /**
     * @param array<int, int> $blocks linear block index => scalar state id
     * @param array<int, BiomeId> $biomes column index => biome
     * @param array<int, int> $extraData linear block index => extra data
     * @param array<int, int> $skyLight linear block index => light
     * @param array<int, int> $blockLight linear block index => light
     */
    public static function apply(
        int $handle,
        ChunkPos $position,
        int $expectedTerrainRevision,
        int $nextTerrainRevision,
        int $expectedLightRevision,
        int $nextLightRevision,
        array $blocks,
        array $biomes,
        array $extraData,
        array $skyLight,
        array $blockLight,
    ): void {
        $payload = pack(
            'P4',
            $expectedTerrainRevision,
            $nextTerrainRevision,
            $expectedLightRevision,
            $nextLightRevision,
        ) . pack(
            'V5',
            count($blocks),
            count($biomes),
            count($extraData),
            count($skyLight),
            count($blockLight),
        );
        foreach ($blocks as $index => $stateId) {
            $payload .= pack('vv', $index, $stateId);
        }
        foreach ($biomes as $index => $biome) {
            $payload .= pack('CC', $index, $biome->value);
        }
        foreach ($extraData as $index => $value) {
            $payload .= pack('vv', $index, $value);
        }
        foreach ($skyLight as $index => $level) {
            $payload .= pack('vC', $index, $level);
        }
        foreach ($blockLight as $index => $level) {
            $payload .= pack('vC', $index, $level);
        }

        cobblestone_world_apply_patch(
            $handle,
            $position->x,
            $position->z,
            $payload,
        );
    }
}
