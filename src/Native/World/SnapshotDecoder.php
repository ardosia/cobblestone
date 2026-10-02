<?php

declare(strict_types=1);

namespace Cobblestone\Native\World;

use Cobblestone\World\ChunkPos;
use Cobblestone\World\ChunkSnapshot;

/**
 * Decodes the fixed native chunk snapshot projection into the PHP semantic snapshot.
 *
 * @internal
 */
final class SnapshotDecoder
{
    public static function decode(ChunkPos $position, string $projection): ChunkSnapshot
    {
        $offset = 0;
        $take = static function (string $bytes, int &$offset, int $length): string {
            $value = substr($bytes, $offset, $length);
            if (strlen($value) !== $length) {
                throw new \UnexpectedValueException('native chunk snapshot projection is truncated');
            }
            $offset += $length;

            return $value;
        };

        $terrainParts = unpack('Pvalue', $take($projection, $offset, 8));
        $lightParts = unpack('Pvalue', $take($projection, $offset, 8));
        if ($terrainParts === false || $lightParts === false) {
            throw new \UnexpectedValueException('native chunk snapshot revision header is invalid');
        }

        $terrainRevision = $terrainParts['value'];
        $lightRevision = $lightParts['value'];
        if (
            !is_int($terrainRevision)
            || $terrainRevision < 0
            || !is_int($lightRevision)
            || $lightRevision < 0
        ) {
            throw new \UnexpectedValueException('native chunk snapshot revision exceeds PHP integer range');
        }

        $blockIds = $take($projection, $offset, ChunkSnapshot::BLOCK_COUNT);
        $blockData = $take($projection, $offset, ChunkSnapshot::NIBBLE_BYTES);
        $skyLight = $take($projection, $offset, ChunkSnapshot::NIBBLE_BYTES);
        $blockLight = $take($projection, $offset, ChunkSnapshot::NIBBLE_BYTES);
        $biomes = $take($projection, $offset, ChunkSnapshot::COLUMN_COUNT);
        $heightMap = $take($projection, $offset, ChunkSnapshot::COLUMN_COUNT);

        $countParts = unpack('Vvalue', $take($projection, $offset, 4));
        if ($countParts === false || !is_int($countParts['value'])) {
            throw new \UnexpectedValueException('native chunk snapshot extra-data count is invalid');
        }
        $extraCount = $countParts['value'];
        $remaining = strlen($projection) - $offset;
        if ($extraCount > intdiv($remaining, 4) || $remaining !== $extraCount * 4) {
            throw new \UnexpectedValueException('native chunk snapshot extra-data payload length mismatch');
        }

        $extraData = [];
        for ($entry = 0; $entry < $extraCount; ++$entry) {
            $parts = unpack('vkey/vvalue', $take($projection, $offset, 4));
            if ($parts === false) {
                throw new \UnexpectedValueException('native chunk snapshot extra-data entry is invalid');
            }
            $extraData[$parts['key']] = $parts['value'];
        }

        return new ChunkSnapshot(
            $position,
            $terrainRevision,
            $blockIds,
            $blockData,
            $skyLight,
            $blockLight,
            $biomes,
            $heightMap,
            $extraData,
            $lightRevision,
        );
    }
}
