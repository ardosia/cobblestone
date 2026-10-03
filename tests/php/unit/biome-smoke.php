<?php

declare(strict_types=1);

use Cobblestone\World\BiomeCatalog;
use Cobblestone\World\BiomeColumn;
use Cobblestone\World\BiomeId;
use Cobblestone\World\Generator\FlatPreset;

require dirname(__DIR__, 3) . '/vendor/autoload.php';

function biomeExpect(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

$ids = BiomeCatalog::ids();
biomeExpect(count($ids) === 60, 'fixed-target biome catalog count mismatch');
biomeExpect($ids[0] === 0 && $ids[array_key_last($ids)] === 167, 'biome catalog ordering mismatch');
biomeExpect(!in_array(9, $ids, true), 'dormant End biome must not be registered as a 0.15.10 world biome');
biomeExpect(!in_array(44, $ids, true), 'post-0.15 ocean variants must not leak into the catalog');
biomeExpect(!in_array(168, $ids, true), 'post-0.15 bamboo biomes must not leak into the catalog');

foreach ($ids as $id) {
    $biome = new BiomeId($id);
    biomeExpect($biome->name() !== '', "biome {$id} has no stable name");
    biomeExpect($biome->defaultColor() >= 0 && $biome->defaultColor() <= 0xffffff, "biome {$id} color out of range");

    $column = $biome->column();
    biomeExpect($column->id->value === $id, "biome {$id} column id mismatch");
    biomeExpect(BiomeColumn::fromWord($column->word()) == $column, "biome {$id} word did not round-trip");
}

biomeExpect((new BiomeId(BiomeId::PLAINS))->defaultColor() === 0x92bc59, 'Plains fixed-target color changed');
biomeExpect((new BiomeId(BiomeId::SWAMPLAND))->defaultColor() === 0x6a7039, 'Swampland fixed-target color mismatch');
biomeExpect((new BiomeId(37))->defaultColor() === 0x90814d, 'Mesa fixed-target color mismatch');
biomeExpect((new BiomeId(BiomeId::HELL))->defaultColor() === 0, 'Hell fixed-target color mismatch');

$custom = new BiomeColumn(new BiomeId(BiomeId::FOREST), 0x123456);
biomeExpect($custom->word() === 0x04123456, 'biome word layout mismatch');
biomeExpect($custom->withId(new BiomeId(BiomeId::DESERT))->word() === 0x02123456, 'biome id edit did not preserve color');
biomeExpect($custom->withColor(0xabcdef)->word() === 0x04abcdef, 'biome color edit did not preserve id');

foreach ([9, 40, 44, 127, 168, 255] as $unsupported) {
    try {
        new BiomeId($unsupported);
        throw new RuntimeException("unsupported biome {$unsupported} was accepted");
    } catch (ValueError) {
    }
}

try {
    FlatPreset::parse('2;7,2x3,2;9;');
    throw new RuntimeException('flat preset accepted dormant End biome');
} catch (ValueError) {
}

echo "biome-smoke: passed\n";
