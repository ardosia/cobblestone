<?php

declare(strict_types=1);

require dirname(__DIR__) . '/bootstrap.php';

use Cobblestone\World\BiomeId;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\Generator\OverworldBiomeSource;

function nativeBiomeSourceExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

function nativeBiomeSourceHash(string $ids): string
{
    // Rust fixtures use exact u64 FNV-1a. PHP verifies the same independent oracle bytes with
    // SHA-256 to avoid unsigned-u64 arithmetic differences at the language boundary.
    return hash('sha256', $ids);
}

nativeBiomeSourceExpect(
    function_exists('cobblestone_world_overworld_biomes'),
    'native Overworld biome-source export is missing',
);

$fixtures = [
    [0, 0, 0, 'ce82812d0dbffa772d5c0bb18bf60f68f8c40d3cd2f817b71ec274a4c9017fde'],
    [1, 0, 0, '5341e6b2646979a70e57653007a1f310169421ec9bdd9f1a5648f75ade005af1'],
    [-1, -16, -16, '8e36e19f581188850ab74cc6bf867ae0f8303b86f6687126775cb67fb41b980f'],
    [0x12345678, 1024, -2048, '26dbdb9e685ea5b8dfd5dabba790b373bf1256faaef13a66b3f4ce3438e9f4ed'],
];

foreach ($fixtures as [$seed, $x, $z, $expectedHash]) {
    $area = (new OverworldBiomeSource($seed))->area($x, $z, 16, 16);
    nativeBiomeSourceExpect(strlen($area->binaryIds()) === 256, 'native biome area length mismatch');
    nativeBiomeSourceExpect(
        nativeBiomeSourceHash($area->binaryIds()) === $expectedHash,
        "native biome fixture mismatch seed={$seed} at {$x}:{$z}",
    );
}

$source = new OverworldBiomeSource(42);
$chunk = $source->chunk(new ChunkPos(-17, 16));
nativeBiomeSourceExpect(
    [$chunk->originX, $chunk->originZ, $chunk->width, $chunk->height] === [-272, 256, 16, 16],
    'chunk biome area coordinates mismatch',
);
$id = $chunk->idAt(0, 0);
nativeBiomeSourceExpect($id instanceof BiomeId, 'chunk biome access did not produce BiomeId');
nativeBiomeSourceExpect(
    $chunk->columnAt(0, 0)->color === $id->defaultColor(),
    'generated biome column did not use catalog fixed-target default color',
);
nativeBiomeSourceExpect(
    $source->biomeAt(-272, 256)->value === $id->value,
    'single-cell and chunk biome source paths disagree',
);

foreach (
    [
        static fn () => new OverworldBiomeSource(0x80000000),
        static fn () => $source->area(0, 0, 0, 1),
        static fn () => $source->area(0, 0, 65, 1),
        static fn () => $source->area(0x7fffffff, 0, 2, 1),
    ] as $invalid
) {
    try {
        $invalid();
        throw new RuntimeException('invalid Overworld biome-source input was accepted');
    } catch (ValueError) {
    }
}

fwrite(STDOUT, "native-biome-source-smoke: passed\n");
