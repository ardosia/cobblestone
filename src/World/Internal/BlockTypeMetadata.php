<?php

declare(strict_types=1);

namespace Cobblestone\World\Internal;

use Cobblestone\World\BlockType;
use Cobblestone\World\Light\BlockLightProperties;

/** @internal Recovered static light metadata for BlockType. */
final class BlockTypeMetadata
{
    private function __construct() {}

    public static function lightProperties(BlockType $type): BlockLightProperties
    {
        /** @var array<int, BlockLightProperties> $cache */
        static $cache = [];

        return $cache[$type->value] ??= new BlockLightProperties(
            self::lightBlock($type),
            self::lightEmission($type),
        );
    }

    private static function lightBlock(BlockType $type): int
    {
        return match ($type) {
            BlockType::Stone,
            BlockType::Grass,
            BlockType::Dirt,
            BlockType::Cobblestone,
            BlockType::Planks,
            BlockType::Bedrock,
            BlockType::Sand,
            BlockType::Gravel,
            BlockType::GoldOre,
            BlockType::IronOre,
            BlockType::CoalOre,
            BlockType::Log,
            BlockType::SpongeDry,
            BlockType::LapisOre,
            BlockType::LapisBlock,
            BlockType::Dispenser,
            BlockType::Sandstone,
            BlockType::NoteBlock,
            BlockType::StickyPiston,
            BlockType::Piston,
            BlockType::PistonArmCollision,
            BlockType::Wool,
            BlockType::GoldBlock,
            BlockType::IronBlock,
            BlockType::DoubleStoneSlab,
            BlockType::BrickBlock,
            BlockType::Tnt,
            BlockType::Bookshelf,
            BlockType::MossyCobblestone,
            BlockType::Obsidian,
            BlockType::MobSpawner,
            BlockType::DiamondOre,
            BlockType::DiamondBlock,
            BlockType::CraftingTable,
            BlockType::Furnace,
            BlockType::LitFurnace,
            BlockType::RedstoneOre,
            BlockType::LitRedstoneOre,
            BlockType::Snow,
            BlockType::Clay,
            BlockType::Pumpkin,
            BlockType::Netherrack,
            BlockType::SoulSand,
            BlockType::Glowstone,
            BlockType::LitPumpkin,
            BlockType::MonsterEgg,
            BlockType::StoneBrick,
            BlockType::BrownMushroomBlock,
            BlockType::RedMushroomBlock,
            BlockType::MelonBlock,
            BlockType::Mycelium,
            BlockType::NetherBrick,
            BlockType::EndStone,
            BlockType::RedstoneLamp,
            BlockType::LitRedstoneLamp,
            BlockType::Dropper,
            BlockType::EmeraldOre,
            BlockType::EmeraldBlock,
            BlockType::RedstoneBlock,
            BlockType::QuartzOre,
            BlockType::QuartzBlock,
            BlockType::DoubleWoodenSlab,
            BlockType::StainedHardenedClay,
            BlockType::Log2,
            BlockType::Slime,
            BlockType::HayBlock,
            BlockType::HardenedClay,
            BlockType::CoalBlock,
            BlockType::PackedIce,
            BlockType::RedSandstone,
            BlockType::DoubleStoneSlab2,
            BlockType::Podzol,
            BlockType::Stonecutter,
            BlockType::GlowingObsidian,
            BlockType::NetherReactor,
            BlockType::InfoUpdate,
            BlockType::InfoUpdate2,
            BlockType::Reserved6
                => 15,
            default => 0,
        };
    }

    private static function lightEmission(BlockType $type): int
    {
        return match ($type) {
            BlockType::FlowingLava,
            BlockType::Lava,
            BlockType::Fire,
            BlockType::Glowstone,
            BlockType::LitPumpkin,
            BlockType::LitRedstoneLamp => 15,
            BlockType::Torch => 14,
            BlockType::LitFurnace,
            BlockType::GlowingObsidian => 13,
            BlockType::Portal => 11,
            BlockType::LitRedstoneOre => 9,
            BlockType::RedstoneTorch,
            BlockType::PoweredRepeater,
            BlockType::PoweredComparator => 7,
            BlockType::BrownMushroom => 1,
            default => 0,
        };
    }
}
