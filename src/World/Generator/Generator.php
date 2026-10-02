<?php

declare(strict_types=1);

namespace Cobblestone\World\Generator;

use Cobblestone\World\BlockPos;
use Cobblestone\World\Chunk;
use Cobblestone\World\ChunkPos;
use Cobblestone\Native\World as NativeWorld;

interface Generator
{
    public function name(): string;

    public function type(): GeneratorType;

    /** @return array<string, mixed> */
    public function settings(): array;

    public function generate(
        ChunkPos $position,
        int $seed,
        ?NativeWorld $nativeStore = null,
    ): Chunk;

    public function populate(Chunk $chunk, int $seed): void;

    public function spawn(): BlockPos;
}
