<?php

declare(strict_types=1);

namespace Cobblestone\World\Generator;

use Cobblestone\World\BiomeArea;
use Cobblestone\World\BiomeId;
use Cobblestone\World\ChunkPos;

interface BiomeSource
{
    public function biomeAt(int $x, int $z): BiomeId;

    public function area(int $x, int $z, int $width, int $height): BiomeArea;

    public function chunk(ChunkPos $position): BiomeArea;
}
