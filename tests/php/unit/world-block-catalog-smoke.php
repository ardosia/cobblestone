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

/** @var array<int, string> $expected */
$expected = [
    0 => 'air',
    1 => 'stone',
    2 => 'grass',
    3 => 'dirt',
    4 => 'cobblestone',
    5 => 'planks',
    6 => 'sapling',
    7 => 'bedrock',
    8 => 'flowing_water',
    9 => 'water',
    10 => 'flowing_lava',
    11 => 'lava',
    12 => 'sand',
    13 => 'gravel',
    14 => 'gold_ore',
    15 => 'iron_ore',
    16 => 'coal_ore',
    17 => 'log',
    18 => 'leaves',
    19 => 'sponge.dry',
    20 => 'glass',
    21 => 'lapis_ore',
    22 => 'lapis_block',
    23 => 'dispenser',
    24 => 'sandstone',
    25 => 'noteblock',
    26 => 'bed',
    27 => 'golden_rail',
    28 => 'detector_rail',
    29 => 'sticky_piston',
    30 => 'web',
    31 => 'tallgrass',
    32 => 'deadbush',
    33 => 'piston',
    34 => 'pistonArmCollision',
    35 => 'wool',
    37 => 'yellow_flower',
    38 => 'red_flower',
    39 => 'brown_mushroom',
    40 => 'red_mushroom',
    41 => 'gold_block',
    42 => 'iron_block',
    43 => 'double_stone_slab',
    44 => 'stone_slab',
    45 => 'brick_block',
    46 => 'tnt',
    47 => 'bookshelf',
    48 => 'mossy_cobblestone',
    49 => 'obsidian',
    50 => 'torch',
    51 => 'fire',
    52 => 'mob_spawner',
    53 => 'oak_stairs',
    54 => 'chest',
    55 => 'redstone_wire',
    56 => 'diamond_ore',
    57 => 'diamond_block',
    58 => 'crafting_table',
    59 => 'wheat',
    60 => 'farmland',
    61 => 'furnace',
    62 => 'lit_furnace',
    63 => 'standing_sign',
    64 => 'wooden_door',
    65 => 'ladder',
    66 => 'rail',
    67 => 'stone_stairs',
    68 => 'wall_sign',
    69 => 'lever',
    70 => 'stone_pressure_plate',
    71 => 'iron_door',
    72 => 'wooden_pressure_plate',
    73 => 'redstone_ore',
    74 => 'lit_redstone_ore',
    75 => 'unlit_redstone_torch',
    76 => 'redstone_torch',
    77 => 'stone_button',
    78 => 'snow_layer',
    79 => 'ice',
    80 => 'snow',
    81 => 'cactus',
    82 => 'clay',
    83 => 'reeds',
    85 => 'fence',
    86 => 'pumpkin',
    87 => 'netherrack',
    88 => 'soul_sand',
    89 => 'glowstone',
    90 => 'portal',
    91 => 'lit_pumpkin',
    92 => 'cake',
    93 => 'unpowered_repeater',
    94 => 'powered_repeater',
    95 => 'invisibleBedrock',
    96 => 'trapdoor',
    97 => 'monster_egg',
    98 => 'stonebrick',
    99 => 'brown_mushroom_block',
    100 => 'red_mushroom_block',
    101 => 'iron_bars',
    102 => 'glass_pane',
    103 => 'melon_block',
    104 => 'pumpkin_stem',
    105 => 'melon_stem',
    106 => 'vine',
    107 => 'fence_gate',
    108 => 'brick_stairs',
    109 => 'stone_brick_stairs',
    110 => 'mycelium',
    111 => 'waterlily',
    112 => 'nether_brick',
    113 => 'nether_brick_fence',
    114 => 'nether_brick_stairs',
    115 => 'nether_wart',
    116 => 'enchanting_table',
    117 => 'brewing_stand',
    118 => 'cauldron',
    120 => 'end_portal_frame',
    121 => 'end_stone',
    123 => 'redstone_lamp',
    124 => 'lit_redstone_lamp',
    125 => 'dropper',
    126 => 'activator_rail',
    127 => 'cocoa',
    128 => 'sandstone_stairs',
    129 => 'emerald_ore',
    131 => 'tripwire_hook',
    132 => 'tripWire',
    133 => 'emerald_block',
    134 => 'spruce_stairs',
    135 => 'birch_stairs',
    136 => 'jungle_stairs',
    139 => 'cobblestone_wall',
    140 => 'flower_pot',
    141 => 'carrots',
    142 => 'potatoes',
    143 => 'wooden_button',
    144 => 'skull',
    145 => 'anvil',
    146 => 'trapped_chest',
    147 => 'light_weighted_pressure_plate',
    148 => 'heavy_weighted_pressure_plate',
    149 => 'unpowered_comparator',
    150 => 'powered_comparator',
    151 => 'daylight_detector',
    152 => 'redstone_block',
    153 => 'quartz_ore',
    154 => 'hopper',
    155 => 'quartz_block',
    156 => 'quartz_stairs',
    157 => 'double_wooden_slab',
    158 => 'wooden_slab',
    159 => 'stained_hardened_clay',
    161 => 'leaves2',
    162 => 'log2',
    163 => 'acacia_stairs',
    164 => 'dark_oak_stairs',
    165 => 'slime',
    167 => 'iron_trapdoor',
    170 => 'hay_block',
    171 => 'carpet',
    172 => 'hardened_clay',
    173 => 'coal_block',
    174 => 'packed_ice',
    175 => 'double_plant',
    178 => 'daylight_detector_inverted',
    179 => 'red_sandstone',
    180 => 'red_sandstone_stairs',
    181 => 'double_stone_slab2',
    182 => 'stone_slab2',
    183 => 'spruce_fence_gate',
    184 => 'birch_fence_gate',
    185 => 'jungle_fence_gate',
    186 => 'dark_oak_fence_gate',
    187 => 'acacia_fence_gate',
    193 => 'spruce_door',
    194 => 'birch_door',
    195 => 'jungle_door',
    196 => 'acacia_door',
    197 => 'dark_oak_door',
    198 => 'grass_path',
    199 => 'frame',
    243 => 'podzol',
    244 => 'beetroot',
    245 => 'stonecutter',
    246 => 'glowingobsidian',
    247 => 'netherreactor',
    248 => 'info_update',
    249 => 'info_update2',
    250 => 'movingBlock',
    255 => 'reserved6',
];

blockCatalogExpect(count($expected) === 191, 'fixed-target block fixture must contain 191 identities');
blockCatalogExpect(
    array_map(static fn (BlockType $type): int => $type->value, BlockType::cases()) === array_keys($expected),
    'BlockType legacy id set/order mismatch',
);

foreach ($expected as $id => $name) {
    $type = BlockType::tryFrom($id);
    blockCatalogExpect($type !== null, "registered block id {$id} was rejected");
    blockCatalogExpect($type->assetName() === $name, "block {$id} asset name mismatch");
    blockCatalogExpect(BlockType::fromAssetName($name) === $type, "block name {$name} did not round-trip");

    $state = $type->state(BlockData::Fifteen);
    blockCatalogExpect($state->stateId() === (($id << 4) | 15), "block {$id}:15 state mismatch");

    $decoded = BlockState::fromId($state->stateId());
    blockCatalogExpect($decoded->type === $type, "block {$id}:15 type round-trip mismatch");
    blockCatalogExpect($decoded->data === BlockData::Fifteen, "block {$id}:15 data round-trip mismatch");
}

for ($id = 0; $id <= 255; ++$id) {
    if (isset($expected[$id])) {
        continue;
    }

    blockCatalogExpect(BlockType::tryFrom($id) === null, "hole block id {$id} was registered");
    try {
        BlockStateId::assert($id << 4);
        throw new RuntimeException("hole block id {$id} produced a state");
    } catch (ValueError) {
    }
}

blockCatalogExpect(
    array_map(static fn (BlockData $data): int => $data->value, BlockData::cases()) === range(0, 15),
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
blockCatalogExpect($torch->value === 50 && $torch->assetName() === 'torch', 'torch identity mismatch');
blockCatalogExpect(
    $torch->lightProperties()->lightBlock === 0 && $torch->lightProperties()->lightEmission === 14,
    'torch light metadata mismatch',
);
blockCatalogExpect(BlockType::Fire->value === 51, 'asset-order fire id mismatch');
blockCatalogExpect(BlockType::NetherBrickFence->value === 113, 'sparse registry id mapping mismatch');
blockCatalogExpect(
    BlockType::Torch->lightProperties() === BlockType::Torch->lightProperties(),
    'block light properties should be cached per semantic block identity',
);

fwrite(STDOUT, "world-block-catalog-smoke: passed\n");
