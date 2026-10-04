<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Exact registered block identities for Minecraft: Windows 10 Edition Beta 0.15.10.
 *
 * Names preserve the shipped vanilla blocks.json registry vocabulary verbatim. Light metadata is
 * the recovered fixed-target light-block/emission data keyed by the same legacy block identities.
 */
final class BlockCatalog
{
    /** @var array<int, array{name: string, light_block: int, light_emission: int}> */
    private const BLOCKS = [
        0 => ['name' => 'air', 'light_block' => 0, 'light_emission' => 0],
        1 => ['name' => 'stone', 'light_block' => 15, 'light_emission' => 0],
        2 => ['name' => 'grass', 'light_block' => 15, 'light_emission' => 0],
        3 => ['name' => 'dirt', 'light_block' => 15, 'light_emission' => 0],
        4 => ['name' => 'cobblestone', 'light_block' => 15, 'light_emission' => 0],
        5 => ['name' => 'planks', 'light_block' => 15, 'light_emission' => 0],
        6 => ['name' => 'sapling', 'light_block' => 0, 'light_emission' => 0],
        7 => ['name' => 'bedrock', 'light_block' => 15, 'light_emission' => 0],
        8 => ['name' => 'flowing_water', 'light_block' => 0, 'light_emission' => 0],
        9 => ['name' => 'water', 'light_block' => 0, 'light_emission' => 0],
        10 => ['name' => 'flowing_lava', 'light_block' => 0, 'light_emission' => 15],
        11 => ['name' => 'lava', 'light_block' => 0, 'light_emission' => 15],
        12 => ['name' => 'sand', 'light_block' => 15, 'light_emission' => 0],
        13 => ['name' => 'gravel', 'light_block' => 15, 'light_emission' => 0],
        14 => ['name' => 'gold_ore', 'light_block' => 15, 'light_emission' => 0],
        15 => ['name' => 'iron_ore', 'light_block' => 15, 'light_emission' => 0],
        16 => ['name' => 'coal_ore', 'light_block' => 15, 'light_emission' => 0],
        17 => ['name' => 'log', 'light_block' => 15, 'light_emission' => 0],
        18 => ['name' => 'leaves', 'light_block' => 0, 'light_emission' => 0],
        19 => ['name' => 'sponge.dry', 'light_block' => 15, 'light_emission' => 0],
        20 => ['name' => 'glass', 'light_block' => 0, 'light_emission' => 0],
        21 => ['name' => 'lapis_ore', 'light_block' => 15, 'light_emission' => 0],
        22 => ['name' => 'lapis_block', 'light_block' => 15, 'light_emission' => 0],
        23 => ['name' => 'dispenser', 'light_block' => 15, 'light_emission' => 0],
        24 => ['name' => 'sandstone', 'light_block' => 15, 'light_emission' => 0],
        25 => ['name' => 'noteblock', 'light_block' => 15, 'light_emission' => 0],
        26 => ['name' => 'bed', 'light_block' => 0, 'light_emission' => 0],
        27 => ['name' => 'golden_rail', 'light_block' => 0, 'light_emission' => 0],
        28 => ['name' => 'detector_rail', 'light_block' => 0, 'light_emission' => 0],
        29 => ['name' => 'sticky_piston', 'light_block' => 15, 'light_emission' => 0],
        30 => ['name' => 'web', 'light_block' => 0, 'light_emission' => 0],
        31 => ['name' => 'tallgrass', 'light_block' => 0, 'light_emission' => 0],
        32 => ['name' => 'deadbush', 'light_block' => 0, 'light_emission' => 0],
        33 => ['name' => 'piston', 'light_block' => 15, 'light_emission' => 0],
        34 => ['name' => 'pistonArmCollision', 'light_block' => 15, 'light_emission' => 0],
        35 => ['name' => 'wool', 'light_block' => 15, 'light_emission' => 0],
        37 => ['name' => 'yellow_flower', 'light_block' => 0, 'light_emission' => 0],
        38 => ['name' => 'red_flower', 'light_block' => 0, 'light_emission' => 0],
        39 => ['name' => 'brown_mushroom', 'light_block' => 0, 'light_emission' => 1],
        40 => ['name' => 'red_mushroom', 'light_block' => 0, 'light_emission' => 0],
        41 => ['name' => 'gold_block', 'light_block' => 15, 'light_emission' => 0],
        42 => ['name' => 'iron_block', 'light_block' => 15, 'light_emission' => 0],
        43 => ['name' => 'double_stone_slab', 'light_block' => 15, 'light_emission' => 0],
        44 => ['name' => 'stone_slab', 'light_block' => 0, 'light_emission' => 0],
        45 => ['name' => 'brick_block', 'light_block' => 15, 'light_emission' => 0],
        46 => ['name' => 'tnt', 'light_block' => 15, 'light_emission' => 0],
        47 => ['name' => 'bookshelf', 'light_block' => 15, 'light_emission' => 0],
        48 => ['name' => 'mossy_cobblestone', 'light_block' => 15, 'light_emission' => 0],
        49 => ['name' => 'obsidian', 'light_block' => 15, 'light_emission' => 0],
        50 => ['name' => 'torch', 'light_block' => 0, 'light_emission' => 14],
        51 => ['name' => 'fire', 'light_block' => 0, 'light_emission' => 15],
        52 => ['name' => 'mob_spawner', 'light_block' => 15, 'light_emission' => 0],
        53 => ['name' => 'oak_stairs', 'light_block' => 0, 'light_emission' => 0],
        54 => ['name' => 'chest', 'light_block' => 0, 'light_emission' => 0],
        55 => ['name' => 'redstone_wire', 'light_block' => 0, 'light_emission' => 0],
        56 => ['name' => 'diamond_ore', 'light_block' => 15, 'light_emission' => 0],
        57 => ['name' => 'diamond_block', 'light_block' => 15, 'light_emission' => 0],
        58 => ['name' => 'crafting_table', 'light_block' => 15, 'light_emission' => 0],
        59 => ['name' => 'wheat', 'light_block' => 0, 'light_emission' => 0],
        60 => ['name' => 'farmland', 'light_block' => 0, 'light_emission' => 0],
        61 => ['name' => 'furnace', 'light_block' => 15, 'light_emission' => 0],
        62 => ['name' => 'lit_furnace', 'light_block' => 15, 'light_emission' => 13],
        63 => ['name' => 'standing_sign', 'light_block' => 0, 'light_emission' => 0],
        64 => ['name' => 'wooden_door', 'light_block' => 0, 'light_emission' => 0],
        65 => ['name' => 'ladder', 'light_block' => 0, 'light_emission' => 0],
        66 => ['name' => 'rail', 'light_block' => 0, 'light_emission' => 0],
        67 => ['name' => 'stone_stairs', 'light_block' => 0, 'light_emission' => 0],
        68 => ['name' => 'wall_sign', 'light_block' => 0, 'light_emission' => 0],
        69 => ['name' => 'lever', 'light_block' => 0, 'light_emission' => 0],
        70 => ['name' => 'stone_pressure_plate', 'light_block' => 0, 'light_emission' => 0],
        71 => ['name' => 'iron_door', 'light_block' => 0, 'light_emission' => 0],
        72 => ['name' => 'wooden_pressure_plate', 'light_block' => 0, 'light_emission' => 0],
        73 => ['name' => 'redstone_ore', 'light_block' => 15, 'light_emission' => 0],
        74 => ['name' => 'lit_redstone_ore', 'light_block' => 15, 'light_emission' => 9],
        75 => ['name' => 'unlit_redstone_torch', 'light_block' => 0, 'light_emission' => 0],
        76 => ['name' => 'redstone_torch', 'light_block' => 0, 'light_emission' => 7],
        77 => ['name' => 'stone_button', 'light_block' => 0, 'light_emission' => 0],
        78 => ['name' => 'snow_layer', 'light_block' => 0, 'light_emission' => 0],
        79 => ['name' => 'ice', 'light_block' => 0, 'light_emission' => 0],
        80 => ['name' => 'snow', 'light_block' => 15, 'light_emission' => 0],
        81 => ['name' => 'cactus', 'light_block' => 0, 'light_emission' => 0],
        82 => ['name' => 'clay', 'light_block' => 15, 'light_emission' => 0],
        83 => ['name' => 'reeds', 'light_block' => 0, 'light_emission' => 0],
        85 => ['name' => 'fence', 'light_block' => 0, 'light_emission' => 0],
        86 => ['name' => 'pumpkin', 'light_block' => 15, 'light_emission' => 0],
        87 => ['name' => 'netherrack', 'light_block' => 15, 'light_emission' => 0],
        88 => ['name' => 'soul_sand', 'light_block' => 15, 'light_emission' => 0],
        89 => ['name' => 'glowstone', 'light_block' => 15, 'light_emission' => 15],
        90 => ['name' => 'portal', 'light_block' => 0, 'light_emission' => 11],
        91 => ['name' => 'lit_pumpkin', 'light_block' => 15, 'light_emission' => 15],
        92 => ['name' => 'cake', 'light_block' => 0, 'light_emission' => 0],
        93 => ['name' => 'unpowered_repeater', 'light_block' => 0, 'light_emission' => 0],
        94 => ['name' => 'powered_repeater', 'light_block' => 0, 'light_emission' => 7],
        95 => ['name' => 'invisibleBedrock', 'light_block' => 0, 'light_emission' => 0],
        96 => ['name' => 'trapdoor', 'light_block' => 0, 'light_emission' => 0],
        97 => ['name' => 'monster_egg', 'light_block' => 15, 'light_emission' => 0],
        98 => ['name' => 'stonebrick', 'light_block' => 15, 'light_emission' => 0],
        99 => ['name' => 'brown_mushroom_block', 'light_block' => 15, 'light_emission' => 0],
        100 => ['name' => 'red_mushroom_block', 'light_block' => 15, 'light_emission' => 0],
        101 => ['name' => 'iron_bars', 'light_block' => 0, 'light_emission' => 0],
        102 => ['name' => 'glass_pane', 'light_block' => 0, 'light_emission' => 0],
        103 => ['name' => 'melon_block', 'light_block' => 15, 'light_emission' => 0],
        104 => ['name' => 'pumpkin_stem', 'light_block' => 0, 'light_emission' => 0],
        105 => ['name' => 'melon_stem', 'light_block' => 0, 'light_emission' => 0],
        106 => ['name' => 'vine', 'light_block' => 0, 'light_emission' => 0],
        107 => ['name' => 'fence_gate', 'light_block' => 0, 'light_emission' => 0],
        108 => ['name' => 'brick_stairs', 'light_block' => 0, 'light_emission' => 0],
        109 => ['name' => 'stone_brick_stairs', 'light_block' => 0, 'light_emission' => 0],
        110 => ['name' => 'mycelium', 'light_block' => 15, 'light_emission' => 0],
        111 => ['name' => 'waterlily', 'light_block' => 0, 'light_emission' => 0],
        112 => ['name' => 'nether_brick', 'light_block' => 15, 'light_emission' => 0],
        113 => ['name' => 'nether_brick_fence', 'light_block' => 0, 'light_emission' => 0],
        114 => ['name' => 'nether_brick_stairs', 'light_block' => 0, 'light_emission' => 0],
        115 => ['name' => 'nether_wart', 'light_block' => 0, 'light_emission' => 0],
        116 => ['name' => 'enchanting_table', 'light_block' => 0, 'light_emission' => 0],
        117 => ['name' => 'brewing_stand', 'light_block' => 0, 'light_emission' => 0],
        118 => ['name' => 'cauldron', 'light_block' => 0, 'light_emission' => 0],
        120 => ['name' => 'end_portal_frame', 'light_block' => 0, 'light_emission' => 0],
        121 => ['name' => 'end_stone', 'light_block' => 15, 'light_emission' => 0],
        123 => ['name' => 'redstone_lamp', 'light_block' => 15, 'light_emission' => 0],
        124 => ['name' => 'lit_redstone_lamp', 'light_block' => 15, 'light_emission' => 15],
        125 => ['name' => 'dropper', 'light_block' => 15, 'light_emission' => 0],
        126 => ['name' => 'activator_rail', 'light_block' => 0, 'light_emission' => 0],
        127 => ['name' => 'cocoa', 'light_block' => 0, 'light_emission' => 0],
        128 => ['name' => 'sandstone_stairs', 'light_block' => 0, 'light_emission' => 0],
        129 => ['name' => 'emerald_ore', 'light_block' => 15, 'light_emission' => 0],
        131 => ['name' => 'tripwire_hook', 'light_block' => 0, 'light_emission' => 0],
        132 => ['name' => 'tripWire', 'light_block' => 0, 'light_emission' => 0],
        133 => ['name' => 'emerald_block', 'light_block' => 15, 'light_emission' => 0],
        134 => ['name' => 'spruce_stairs', 'light_block' => 0, 'light_emission' => 0],
        135 => ['name' => 'birch_stairs', 'light_block' => 0, 'light_emission' => 0],
        136 => ['name' => 'jungle_stairs', 'light_block' => 0, 'light_emission' => 0],
        139 => ['name' => 'cobblestone_wall', 'light_block' => 0, 'light_emission' => 0],
        140 => ['name' => 'flower_pot', 'light_block' => 0, 'light_emission' => 0],
        141 => ['name' => 'carrots', 'light_block' => 0, 'light_emission' => 0],
        142 => ['name' => 'potatoes', 'light_block' => 0, 'light_emission' => 0],
        143 => ['name' => 'wooden_button', 'light_block' => 0, 'light_emission' => 0],
        144 => ['name' => 'skull', 'light_block' => 0, 'light_emission' => 0],
        145 => ['name' => 'anvil', 'light_block' => 0, 'light_emission' => 0],
        146 => ['name' => 'trapped_chest', 'light_block' => 0, 'light_emission' => 0],
        147 => ['name' => 'light_weighted_pressure_plate', 'light_block' => 0, 'light_emission' => 0],
        148 => ['name' => 'heavy_weighted_pressure_plate', 'light_block' => 0, 'light_emission' => 0],
        149 => ['name' => 'unpowered_comparator', 'light_block' => 0, 'light_emission' => 0],
        150 => ['name' => 'powered_comparator', 'light_block' => 0, 'light_emission' => 7],
        151 => ['name' => 'daylight_detector', 'light_block' => 0, 'light_emission' => 0],
        152 => ['name' => 'redstone_block', 'light_block' => 15, 'light_emission' => 0],
        153 => ['name' => 'quartz_ore', 'light_block' => 15, 'light_emission' => 0],
        154 => ['name' => 'hopper', 'light_block' => 0, 'light_emission' => 0],
        155 => ['name' => 'quartz_block', 'light_block' => 15, 'light_emission' => 0],
        156 => ['name' => 'quartz_stairs', 'light_block' => 0, 'light_emission' => 0],
        157 => ['name' => 'double_wooden_slab', 'light_block' => 15, 'light_emission' => 0],
        158 => ['name' => 'wooden_slab', 'light_block' => 0, 'light_emission' => 0],
        159 => ['name' => 'stained_hardened_clay', 'light_block' => 15, 'light_emission' => 0],
        161 => ['name' => 'leaves2', 'light_block' => 0, 'light_emission' => 0],
        162 => ['name' => 'log2', 'light_block' => 15, 'light_emission' => 0],
        163 => ['name' => 'acacia_stairs', 'light_block' => 0, 'light_emission' => 0],
        164 => ['name' => 'dark_oak_stairs', 'light_block' => 0, 'light_emission' => 0],
        165 => ['name' => 'slime', 'light_block' => 15, 'light_emission' => 0],
        167 => ['name' => 'iron_trapdoor', 'light_block' => 0, 'light_emission' => 0],
        170 => ['name' => 'hay_block', 'light_block' => 15, 'light_emission' => 0],
        171 => ['name' => 'carpet', 'light_block' => 0, 'light_emission' => 0],
        172 => ['name' => 'hardened_clay', 'light_block' => 15, 'light_emission' => 0],
        173 => ['name' => 'coal_block', 'light_block' => 15, 'light_emission' => 0],
        174 => ['name' => 'packed_ice', 'light_block' => 15, 'light_emission' => 0],
        175 => ['name' => 'double_plant', 'light_block' => 0, 'light_emission' => 0],
        178 => ['name' => 'daylight_detector_inverted', 'light_block' => 0, 'light_emission' => 0],
        179 => ['name' => 'red_sandstone', 'light_block' => 15, 'light_emission' => 0],
        180 => ['name' => 'red_sandstone_stairs', 'light_block' => 0, 'light_emission' => 0],
        181 => ['name' => 'double_stone_slab2', 'light_block' => 15, 'light_emission' => 0],
        182 => ['name' => 'stone_slab2', 'light_block' => 0, 'light_emission' => 0],
        183 => ['name' => 'spruce_fence_gate', 'light_block' => 0, 'light_emission' => 0],
        184 => ['name' => 'birch_fence_gate', 'light_block' => 0, 'light_emission' => 0],
        185 => ['name' => 'jungle_fence_gate', 'light_block' => 0, 'light_emission' => 0],
        186 => ['name' => 'dark_oak_fence_gate', 'light_block' => 0, 'light_emission' => 0],
        187 => ['name' => 'acacia_fence_gate', 'light_block' => 0, 'light_emission' => 0],
        193 => ['name' => 'spruce_door', 'light_block' => 0, 'light_emission' => 0],
        194 => ['name' => 'birch_door', 'light_block' => 0, 'light_emission' => 0],
        195 => ['name' => 'jungle_door', 'light_block' => 0, 'light_emission' => 0],
        196 => ['name' => 'acacia_door', 'light_block' => 0, 'light_emission' => 0],
        197 => ['name' => 'dark_oak_door', 'light_block' => 0, 'light_emission' => 0],
        198 => ['name' => 'grass_path', 'light_block' => 0, 'light_emission' => 0],
        199 => ['name' => 'frame', 'light_block' => 0, 'light_emission' => 0],
        243 => ['name' => 'podzol', 'light_block' => 15, 'light_emission' => 0],
        244 => ['name' => 'beetroot', 'light_block' => 0, 'light_emission' => 0],
        245 => ['name' => 'stonecutter', 'light_block' => 15, 'light_emission' => 0],
        246 => ['name' => 'glowingobsidian', 'light_block' => 15, 'light_emission' => 13],
        247 => ['name' => 'netherreactor', 'light_block' => 15, 'light_emission' => 0],
        248 => ['name' => 'info_update', 'light_block' => 15, 'light_emission' => 0],
        249 => ['name' => 'info_update2', 'light_block' => 15, 'light_emission' => 0],
        250 => ['name' => 'movingBlock', 'light_block' => 0, 'light_emission' => 0],
        255 => ['name' => 'reserved6', 'light_block' => 15, 'light_emission' => 0],
    ];

    private function __construct()
    {
    }

    public static function supports(int $id): bool
    {
        return isset(self::BLOCKS[$id]);
    }

    public static function type(int $id): BlockType
    {
        self::assertSupported($id);

        return new BlockType($id);
    }

    public static function idForName(string $name): int
    {
        foreach (self::BLOCKS as $id => $block) {
            if ($block['name'] === $name) {
                return $id;
            }
        }

        throw new \ValueError("unsupported fixed-target block name '{$name}'");
    }

    public static function name(int $id): string
    {
        self::assertSupported($id);

        return self::BLOCKS[$id]['name'];
    }

    public static function lightBlock(int $id): int
    {
        self::assertSupported($id);

        return self::BLOCKS[$id]['light_block'];
    }

    public static function lightEmission(int $id): int
    {
        self::assertSupported($id);

        return self::BLOCKS[$id]['light_emission'];
    }

    /** @return list<int> */
    public static function ids(): array
    {
        return array_keys(self::BLOCKS);
    }

    private static function assertSupported(int $id): void
    {
        if (!self::supports($id)) {
            throw new \ValueError("unsupported MCPE 0.15.10 block id {$id}");
        }
    }
}
