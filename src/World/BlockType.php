<?php

declare(strict_types=1);

namespace Cobblestone\World;

use Cobblestone\World\Internal\BlockTypeMetadata;
use Cobblestone\World\Light\BlockLightProperties;
use ValueError;

/**
 * Exact registered block identities for Minecraft: Windows 10 Edition Beta 0.15.10.
 *
 * Backed values are the fixed-target legacy block IDs used by protocol/storage state tokens.
 * Enum cases are singleton semantic identities; no BlockType allocation occurs per lookup.
 */
enum BlockType: int
{
    case Air = 0;
    case Stone = 1;
    case Grass = 2;
    case Dirt = 3;
    case Cobblestone = 4;
    case Planks = 5;
    case Sapling = 6;
    case Bedrock = 7;
    case FlowingWater = 8;
    case Water = 9;
    case FlowingLava = 10;
    case Lava = 11;
    case Sand = 12;
    case Gravel = 13;
    case GoldOre = 14;
    case IronOre = 15;
    case CoalOre = 16;
    case Log = 17;
    case Leaves = 18;
    case SpongeDry = 19;
    case Glass = 20;
    case LapisOre = 21;
    case LapisBlock = 22;
    case Dispenser = 23;
    case Sandstone = 24;
    case NoteBlock = 25;
    case Bed = 26;
    case GoldenRail = 27;
    case DetectorRail = 28;
    case StickyPiston = 29;
    case Web = 30;
    case TallGrass = 31;
    case DeadBush = 32;
    case Piston = 33;
    case PistonArmCollision = 34;
    case Wool = 35;
    case YellowFlower = 37;
    case RedFlower = 38;
    case BrownMushroom = 39;
    case RedMushroom = 40;
    case GoldBlock = 41;
    case IronBlock = 42;
    case DoubleStoneSlab = 43;
    case StoneSlab = 44;
    case BrickBlock = 45;
    case Tnt = 46;
    case Bookshelf = 47;
    case MossyCobblestone = 48;
    case Obsidian = 49;
    case Torch = 50;
    case Fire = 51;
    case MobSpawner = 52;
    case OakStairs = 53;
    case Chest = 54;
    case RedstoneWire = 55;
    case DiamondOre = 56;
    case DiamondBlock = 57;
    case CraftingTable = 58;
    case Wheat = 59;
    case Farmland = 60;
    case Furnace = 61;
    case LitFurnace = 62;
    case StandingSign = 63;
    case WoodenDoor = 64;
    case Ladder = 65;
    case Rail = 66;
    case StoneStairs = 67;
    case WallSign = 68;
    case Lever = 69;
    case StonePressurePlate = 70;
    case IronDoor = 71;
    case WoodenPressurePlate = 72;
    case RedstoneOre = 73;
    case LitRedstoneOre = 74;
    case UnlitRedstoneTorch = 75;
    case RedstoneTorch = 76;
    case StoneButton = 77;
    case SnowLayer = 78;
    case Ice = 79;
    case Snow = 80;
    case Cactus = 81;
    case Clay = 82;
    case Reeds = 83;
    case Fence = 85;
    case Pumpkin = 86;
    case Netherrack = 87;
    case SoulSand = 88;
    case Glowstone = 89;
    case Portal = 90;
    case LitPumpkin = 91;
    case Cake = 92;
    case UnpoweredRepeater = 93;
    case PoweredRepeater = 94;
    case InvisibleBedrock = 95;
    case Trapdoor = 96;
    case MonsterEgg = 97;
    case StoneBrick = 98;
    case BrownMushroomBlock = 99;
    case RedMushroomBlock = 100;
    case IronBars = 101;
    case GlassPane = 102;
    case MelonBlock = 103;
    case PumpkinStem = 104;
    case MelonStem = 105;
    case Vine = 106;
    case FenceGate = 107;
    case BrickStairs = 108;
    case StoneBrickStairs = 109;
    case Mycelium = 110;
    case WaterLily = 111;
    case NetherBrick = 112;
    case NetherBrickFence = 113;
    case NetherBrickStairs = 114;
    case NetherWart = 115;
    case EnchantingTable = 116;
    case BrewingStand = 117;
    case Cauldron = 118;
    case EndPortalFrame = 120;
    case EndStone = 121;
    case RedstoneLamp = 123;
    case LitRedstoneLamp = 124;
    case Dropper = 125;
    case ActivatorRail = 126;
    case Cocoa = 127;
    case SandstoneStairs = 128;
    case EmeraldOre = 129;
    case TripwireHook = 131;
    case TripWire = 132;
    case EmeraldBlock = 133;
    case SpruceStairs = 134;
    case BirchStairs = 135;
    case JungleStairs = 136;
    case CobblestoneWall = 139;
    case FlowerPot = 140;
    case Carrots = 141;
    case Potatoes = 142;
    case WoodenButton = 143;
    case Skull = 144;
    case Anvil = 145;
    case TrappedChest = 146;
    case LightWeightedPressurePlate = 147;
    case HeavyWeightedPressurePlate = 148;
    case UnpoweredComparator = 149;
    case PoweredComparator = 150;
    case DaylightDetector = 151;
    case RedstoneBlock = 152;
    case QuartzOre = 153;
    case Hopper = 154;
    case QuartzBlock = 155;
    case QuartzStairs = 156;
    case DoubleWoodenSlab = 157;
    case WoodenSlab = 158;
    case StainedHardenedClay = 159;
    case Leaves2 = 161;
    case Log2 = 162;
    case AcaciaStairs = 163;
    case DarkOakStairs = 164;
    case Slime = 165;
    case IronTrapdoor = 167;
    case HayBlock = 170;
    case Carpet = 171;
    case HardenedClay = 172;
    case CoalBlock = 173;
    case PackedIce = 174;
    case DoublePlant = 175;
    case DaylightDetectorInverted = 178;
    case RedSandstone = 179;
    case RedSandstoneStairs = 180;
    case DoubleStoneSlab2 = 181;
    case StoneSlab2 = 182;
    case SpruceFenceGate = 183;
    case BirchFenceGate = 184;
    case JungleFenceGate = 185;
    case DarkOakFenceGate = 186;
    case AcaciaFenceGate = 187;
    case SpruceDoor = 193;
    case BirchDoor = 194;
    case JungleDoor = 195;
    case AcaciaDoor = 196;
    case DarkOakDoor = 197;
    case GrassPath = 198;
    case Frame = 199;
    case Podzol = 243;
    case Beetroot = 244;
    case Stonecutter = 245;
    case GlowingObsidian = 246;
    case NetherReactor = 247;
    case InfoUpdate = 248;
    case InfoUpdate2 = 249;
    case MovingBlock = 250;
    case Reserved6 = 255;

    public function assetName(): string
    {
        return BlockTypeMetadata::assetName($this);
    }

    public function lightProperties(): BlockLightProperties
    {
        return BlockTypeMetadata::lightProperties($this);
    }

    public function state(BlockData $data = BlockData::Zero): BlockState
    {
        return new BlockState($this, $data);
    }

    public function isAir(): bool
    {
        return $this === self::Air;
    }

    public static function fromAssetName(string $name): self
    {
        return self::tryFromAssetName($name)
            ?? throw new ValueError("unsupported fixed-target block name '{$name}'");
    }

    public static function tryFromAssetName(string $name): ?self
    {
        /** @var array<string, self>|null $byName */
        static $byName = null;

        if ($byName === null) {
            $byName = [];
            foreach (self::cases() as $type) {
                $byName[$type->assetName()] = $type;
            }
        }

        return $byName[$name] ?? null;
    }
}
