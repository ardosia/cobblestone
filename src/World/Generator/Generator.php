<?php

declare(strict_types=1);

namespace Cobblestone\World\Generator;

use Cobblestone\World\BlockPos;
use Cobblestone\World\Chunk;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\NativeWorldStore;

interface Generator
{
    public function name(): string;

    public function type(): GeneratorType;

    /** @return array<string, mixed> */
    public function settings(): array;

    public function generate(
        ChunkPos $position,
        int $seed,
        ?NativeWorldStore $nativeStore = null,
    ): Chunk;

    public function populate(Chunk $chunk, int $seed): void;

    public function spawn(): BlockPos;
}
