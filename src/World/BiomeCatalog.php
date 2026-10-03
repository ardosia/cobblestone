<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Exact registered biome identities for Minecraft: Windows 10 Edition Beta 0.15.10.
 *
 * Colors are the fixed-target default column colors used when generation creates a biome column.
 * Stored biome color remains independent from the biome ID after creation.
 */
final class BiomeCatalog
{
    /** @var array<int, array{name: string, color: int}> */
    private const BIOMES = [
        0 => ['name' => 'Ocean', 'color' => 0x8eb871],
        1 => ['name' => 'Plains', 'color' => 0x92bc59],
        2 => ['name' => 'Desert', 'color' => 0xbfb655],
        3 => ['name' => 'Extreme Hills', 'color' => 0x8ab589],
        4 => ['name' => 'Forest', 'color' => 0x7ac05b],
        5 => ['name' => 'Taiga', 'color' => 0x87b684],
        6 => ['name' => 'Swampland', 'color' => 0x6a7039],
        7 => ['name' => 'River', 'color' => 0x8eb871],
        8 => ['name' => 'Hell', 'color' => 0x000000],
        10 => ['name' => 'Frozen Ocean', 'color' => 0x80b497],
        11 => ['name' => 'Frozen River', 'color' => 0x80b497],
        12 => ['name' => 'Ice Plains', 'color' => 0x80b497],
        13 => ['name' => 'Ice Mountains', 'color' => 0x80b497],
        14 => ['name' => 'Mushroom Island', 'color' => 0x56cb40],
        15 => ['name' => 'Mushroom Island Shore', 'color' => 0x56cb40],
        16 => ['name' => 'Beach', 'color' => 0x92bc59],
        17 => ['name' => 'Desert Hills', 'color' => 0xbfb655],
        18 => ['name' => 'Forest Hills', 'color' => 0x7ac05b],
        19 => ['name' => 'Taiga Hills', 'color' => 0x87b684],
        20 => ['name' => 'Extreme Hills Edge', 'color' => 0x8ab589],
        21 => ['name' => 'Jungle', 'color' => 0x59cb3c],
        22 => ['name' => 'Jungle Hills', 'color' => 0x59cb3c],
        23 => ['name' => 'Jungle Edge', 'color' => 0x64c940],
        24 => ['name' => 'Deep Ocean', 'color' => 0x8eb871],
        25 => ['name' => 'Stone Beach', 'color' => 0x8ab589],
        26 => ['name' => 'Cold Beach', 'color' => 0x83b493],
        27 => ['name' => 'Birch Forest', 'color' => 0x89bb67],
        28 => ['name' => 'Birch Forest Hills', 'color' => 0x89bb67],
        29 => ['name' => 'Roofed Forest', 'color' => 0x517a32],
        30 => ['name' => 'Cold Taiga', 'color' => 0x80b497],
        31 => ['name' => 'Cold Taiga Hills', 'color' => 0x80b497],
        32 => ['name' => 'Mega Taiga', 'color' => 0x87b780],
        33 => ['name' => 'Mega Taiga Hills', 'color' => 0x87b780],
        34 => ['name' => 'Extreme Hills+', 'color' => 0x8ab589],
        35 => ['name' => 'Savanna', 'color' => 0xbfb655],
        36 => ['name' => 'Savanna Plateau', 'color' => 0xbfb655],
        37 => ['name' => 'Mesa', 'color' => 0x90814d],
        38 => ['name' => 'Mesa Plateau F', 'color' => 0x90814d],
        39 => ['name' => 'Mesa Plateau', 'color' => 0x90814d],
        129 => ['name' => 'Sunflower Plains', 'color' => 0x92bc59],
        130 => ['name' => 'Desert M', 'color' => 0xbfb655],
        131 => ['name' => 'Extreme Hills M', 'color' => 0x8ab589],
        132 => ['name' => 'Flower Forest', 'color' => 0x7ac05b],
        133 => ['name' => 'Taiga M', 'color' => 0x87b684],
        134 => ['name' => 'Swampland M', 'color' => 0x6a7039],
        140 => ['name' => 'Ice Plains Spikes', 'color' => 0x80b497],
        149 => ['name' => 'Jungle M', 'color' => 0x59cb3c],
        151 => ['name' => 'Jungle Edge M', 'color' => 0x64c940],
        155 => ['name' => 'Birch Forest M', 'color' => 0x7ac05b],
        156 => ['name' => 'Birch Forest Hills M', 'color' => 0x7ac05b],
        157 => ['name' => 'Roofed Forest M', 'color' => 0x517a32],
        158 => ['name' => 'Cold Taiga M', 'color' => 0x80b497],
        160 => ['name' => 'Mega Spruce Taiga', 'color' => 0x87b684],
        161 => ['name' => 'Mega Spruce Taiga Hills', 'color' => 0x87b780],
        162 => ['name' => 'Extreme Hills+ M', 'color' => 0x8ab589],
        163 => ['name' => 'Savanna M', 'color' => 0x83c344],
        164 => ['name' => 'Savanna Plateau M', 'color' => 0x83c344],
        165 => ['name' => 'Mesa (Bryce)', 'color' => 0x90814d],
        166 => ['name' => 'Mesa Plateau F M', 'color' => 0x90814d],
        167 => ['name' => 'Mesa Plateau M', 'color' => 0x90814d],
    ];

    private function __construct()
    {
    }

    public static function supports(int $id): bool
    {
        return isset(self::BIOMES[$id]);
    }

    public static function name(int $id): string
    {
        self::assertSupported($id);

        return self::BIOMES[$id]['name'];
    }

    public static function defaultColor(int $id): int
    {
        self::assertSupported($id);

        return self::BIOMES[$id]['color'];
    }

    /** @return list<int> */
    public static function ids(): array
    {
        return array_keys(self::BIOMES);
    }

    private static function assertSupported(int $id): void
    {
        if (!self::supports($id)) {
            throw new \ValueError("unsupported fixed-target biome id {$id}");
        }
    }
}
