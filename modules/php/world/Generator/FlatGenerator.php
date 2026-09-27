<?php

declare(strict_types=1);

namespace Cobblestone\World\Generator;

use Cobblestone\World\BlockPos;
use Cobblestone\World\Chunk;
use Cobblestone\World\ChunkPos;
use Cobblestone\World\WorldBounds;

final class FlatGenerator implements Generator
{
    private readonly FlatPreset $preset;

    public function __construct(?FlatPreset $preset = null)
    {
        $this->preset = $preset ?? FlatPreset::default();
    }

    public static function defaults(): self
    {
        return new self(FlatPreset::default());
    }

    public static function fromPreset(string $preset): self
    {
        return new self(FlatPreset::parse($preset));
    }

    public function name(): string
    {
        return 'flat';
    }

    public function type(): GeneratorType
    {
        return GeneratorType::Flat;
    }

    public function settings(): array
    {
        return ['preset' => $this->preset->toString()];
    }

    public function generate(ChunkPos $position, int $seed): Chunk
    {
        $chunk = new Chunk($position, $this->preset->biome());
        $y = 0;

        foreach ($this->preset->layers() as $layer) {
            for ($remaining = $layer->count; $remaining > 0; --$remaining, ++$y) {
                if ($y >= WorldBounds::WORLD_HEIGHT) {
                    break 2;
                }

                for ($z = 0; $z < WorldBounds::CHUNK_EDGE; ++$z) {
                    for ($x = 0; $x < WorldBounds::CHUNK_EDGE; ++$x) {
                        $chunk->setBlock($x, $y, $z, $layer->state);
                    }
                }
            }
        }

        $chunk->fillSkyLightFrom($y, 15);
        $chunk->markGenerated();
        $chunk->markLightPopulated();

        return $chunk;
    }

    public function populate(Chunk $chunk, int $seed): void
    {
        $chunk->markPopulated();
    }

    public function spawn(): BlockPos
    {
        return new BlockPos(
            128,
            min($this->preset->floorLevel(), WorldBounds::MAX_Y),
            128,
        );
    }
}
