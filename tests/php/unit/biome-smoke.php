<?php

declare(strict_types=1);

use Cobblestone\World\BiomeArea;
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
biomeExpect($ids === array_values(array_unique($ids)), 'fixed-target biome ids are duplicated');
$sorted = $ids;
sort($sorted);
biomeExpect($ids === $sorted, 'fixed-target biome ids are not strictly ordered');
biomeExpect($ids[0] === BiomeId::OCEAN && $ids[array_key_last($ids)] === BiomeId::MESA_PLATEAU_M, 'biome catalog boundaries changed');

foreach ($ids as $id) {
    $biome = new BiomeId($id);
    biomeExpect($biome->name() !== '', "biome {$id} has no stable name");
    biomeExpect($biome->defaultColor() >= 0 && $biome->defaultColor() <= 0xffffff, "biome {$id} color out of range");

    $column = $biome->column();
    biomeExpect($column->id->value === $id, "biome {$id} column id mismatch");
    biomeExpect(BiomeColumn::fromWord($column->word()) == $column, "biome {$id} word did not round-trip");
}

foreach ([
    BiomeId::OCEAN => ['Ocean', 0x8eb871],
    BiomeId::PLAINS => ['Plains', 0x92bc59],
    BiomeId::SWAMPLAND => ['Swampland', 0x6a7039],
    BiomeId::HELL => ['Hell', 0x000000],
    BiomeId::MESA => ['Mesa', 0x90814d],
    BiomeId::MESA_PLATEAU_M => ['Mesa Plateau M', 0x90814d],
] as $id => [$name, $color]) {
    $biome = new BiomeId($id);
    biomeExpect($biome->name() === $name, "biome {$id} target name sentinel mismatch");
    biomeExpect($biome->defaultColor() === $color, "biome {$id} target color sentinel mismatch");
}

foreach ([9, 40, 44, 127, 168, 255] as $unsupported) {
    biomeExpect(!in_array($unsupported, $ids, true), "unsupported biome {$unsupported} leaked into catalog");
    try {
        new BiomeId($unsupported);
        throw new RuntimeException("unsupported biome {$unsupported} was accepted");
    } catch (ValueError) {
    }
}

$area = BiomeArea::fromBinary(-1, 2, 2, 2, chr(BiomeId::PLAINS) . chr(BiomeId::DESERT) . chr(BiomeId::FOREST) . chr(BiomeId::TAIGA));
biomeExpect($area->originX === -1 && $area->originZ === 2, 'biome area origin mismatch');
biomeExpect($area->idAt(0, 0)->value === BiomeId::PLAINS, 'biome area first id mismatch');
biomeExpect($area->idAt(1, 0)->value === BiomeId::DESERT, 'biome area x ordering mismatch');
biomeExpect($area->idAt(0, 1)->value === BiomeId::FOREST, 'biome area z ordering mismatch');
biomeExpect(
    $area->columnAt(1, 0)->word() === (new BiomeId(BiomeId::DESERT))->column()->word(),
    'biome area column did not use catalog default color',
);
try {
    $area->idAt(2, 0);
    throw new RuntimeException('biome area accepted out-of-range local coordinate');
} catch (OutOfBoundsException) {
}
try {
    BiomeArea::fromBinary(0, 0, 1, 1, chr(9));
    throw new RuntimeException('biome area accepted unsupported fixed-target biome');
} catch (ValueError) {
}

$custom = new BiomeColumn(new BiomeId(BiomeId::FOREST), 0x123456);
biomeExpect($custom->word() === 0x04123456, 'biome word layout mismatch');
biomeExpect($custom->withId(new BiomeId(BiomeId::DESERT))->word() === 0x02123456, 'biome id edit did not preserve color');
biomeExpect($custom->withColor(0xabcdef)->word() === 0x04abcdef, 'biome color edit did not preserve id');

try {
    FlatPreset::parse('2;7,2x3,2;9;');
    throw new RuntimeException('flat preset accepted dormant End biome');
} catch (ValueError) {
}

echo "biome-smoke: passed\n";
