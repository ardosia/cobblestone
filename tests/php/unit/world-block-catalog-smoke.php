<?php

declare(strict_types=1);

require dirname(__DIR__) . '/bootstrap.php';

use Cobblestone\World\BlockData;
use Cobblestone\World\BlockState;
use Cobblestone\World\BlockStateId;
use Cobblestone\World\BlockType;
use Cobblestone\World\Generator\FlatPreset;

function blockCatalogExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

$types = BlockType::cases();
blockCatalogExpect(count($types) === 191, 'fixed-target public block catalog count mismatch');

$registered = [];
$previous = -1;
foreach ($types as $type) {
    $id = $type->value;
    blockCatalogExpect($id > $previous, "block ids are not strictly ordered near {$id}");
    $previous = $id;
    $registered[$id] = true;

    $asset = $type->assetName();
    blockCatalogExpect($asset !== '', "block {$id} has no asset name");
    blockCatalogExpect(BlockType::fromAssetName($asset) === $type, "block asset {$asset} did not round-trip");

    $state = $type->state(BlockData::Fifteen);
    blockCatalogExpect($state->stateId() === (($id << 4) | 15), "block {$id}:15 state mismatch");

    $decoded = BlockState::fromId($state->stateId());
    blockCatalogExpect($decoded->type === $type, "block {$id}:15 type round-trip mismatch");
    blockCatalogExpect($decoded->data === BlockData::Fifteen, "block {$id}:15 data round-trip mismatch");
}

foreach ([
    0 => 'air',
    19 => 'sponge.dry',
    34 => 'pistonArmCollision',
    50 => 'torch',
    113 => 'nether_brick_fence',
    199 => 'frame',
    246 => 'glowingobsidian',
    250 => 'movingBlock',
    255 => 'reserved6',
] as $id => $asset) {
    $type = BlockType::from($id);
    blockCatalogExpect($type->assetName() === $asset, "block {$id} target asset sentinel mismatch");
}

for ($id = 0; $id <= 255; ++$id) {
    if (isset($registered[$id])) {
        continue;
    }

    blockCatalogExpect(BlockType::tryFrom($id) === null, "hole block id {$id} was registered");
    try {
        BlockStateId::assert($id << 4);
        throw new RuntimeException("hole block id {$id} produced a public state");
    } catch (ValueError) {
    }
}

blockCatalogExpect(BlockType::tryFrom(119) === null, 'executable-only End Portal leaked into public BlockType');
blockCatalogExpect(
    array_map(static fn(BlockData $data): int => $data->value, BlockData::cases()) === range(0, 15),
    'BlockData must cover exactly the legacy nibble',
);

try {
    BlockData::of(16);
    throw new RuntimeException('block metadata 16 was accepted');
} catch (ValueError) {
}

try {
    FlatPreset::parse('2;36;1;');
    throw new RuntimeException('flat preset accepted unsupported block id 36');
} catch (ValueError) {
}

try {
    FlatPreset::parse('2;1:16;1;');
    throw new RuntimeException('flat preset accepted unsupported metadata 16');
} catch (ValueError) {
}

$torch = BlockType::Torch;
blockCatalogExpect(
    $torch->lightProperties()->lightBlock === 0 && $torch->lightProperties()->lightEmission === 14,
    'torch light metadata mismatch',
);
blockCatalogExpect(
    BlockType::Torch->lightProperties() === BlockType::Torch->lightProperties(),
    'block light properties should be cached per semantic block identity',
);

fwrite(STDOUT, "world-block-catalog-smoke: passed\n");
