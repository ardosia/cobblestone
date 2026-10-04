<?php

declare(strict_types=1);

namespace Cobblestone\World\Generator;

use Cobblestone\World\BiomeId;
use Cobblestone\World\BlockData;
use Cobblestone\World\BlockType;
use Cobblestone\World\WorldBounds;
use ValueError;

final class FlatPreset
{
    public const DEFAULT = '2;7,2x3,2;1;';

    /** @var list<FlatLayer> */
    private array $layers;

    /**
     * @param list<FlatLayer> $layers
     */
    private function __construct(
        private readonly int $version,
        array $layers,
        private readonly BiomeId $biome,
        private readonly string $options,
    ) {
        $this->layers = array_values($layers);
    }

    public static function default(): self
    {
        return self::parse(self::DEFAULT);
    }

    public static function parse(string $preset): self
    {
        $parts = explode(';', $preset);
        $version = filter_var($parts[0] ?? null, FILTER_VALIDATE_INT);
        if ($version !== 2) {
            throw new ValueError('only fixed-target flat preset version 2 is supported');
        }

        $layerText = $parts[1] ?? '';
        if ($layerText === '') {
            throw new ValueError('flat preset must contain at least one layer');
        }

        $layers = [];
        $height = 0;
        foreach (explode(',', $layerText) as $token) {
            if (!preg_match('/^(?:(\d+)x)?(\d{1,3})(?::(\d{1,2}))?$/', $token, $matches)) {
                throw new ValueError("invalid fixed-target flat layer '{$token}'");
            }

            $count = isset($matches[1]) && $matches[1] !== '' ? (int) $matches[1] : 1;
            $id = (int) $matches[2];
            $data = isset($matches[3]) && $matches[3] !== '' ? (int) $matches[3] : 0;
            $type = BlockType::tryFrom($id)
                ?? throw new ValueError("unsupported MCPE 0.15.10 block id {$id}");
            $layer = new FlatLayer($count, $type->state(BlockData::of($data)));

            $height += $layer->count;
            if ($height > WorldBounds::WORLD_HEIGHT) {
                throw new ValueError('flat preset layers exceed fixed-target world height');
            }

            $layers[] = $layer;
        }

        $biomeRaw = $parts[2] ?? '1';
        if ($biomeRaw === '' || !ctype_digit($biomeRaw)) {
            throw new ValueError('flat preset biome must be an unsigned integer');
        }

        return new self(
            $version,
            $layers,
            new BiomeId((int) $biomeRaw),
            $parts[3] ?? '',
        );
    }

    public function version(): int
    {
        return $this->version;
    }

    /** @return list<FlatLayer> */
    public function layers(): array
    {
        return $this->layers;
    }

    public function biome(): BiomeId
    {
        return $this->biome;
    }

    public function options(): string
    {
        return $this->options;
    }

    public function floorLevel(): int
    {
        $height = 0;
        foreach ($this->layers as $layer) {
            $height += $layer->count;
        }

        return $height;
    }

    public function toString(): string
    {
        $layers = [];
        foreach ($this->layers as $layer) {
            $prefix = $layer->count === 1 ? '' : $layer->count . 'x';
            $suffix = $layer->state->data === BlockData::Zero ? '' : ':' . $layer->state->data->value;
            $layers[] = $prefix . $layer->state->type->value . $suffix;
        }

        return $this->version
            . ';' . implode(',', $layers)
            . ';' . $this->biome->value
            . ';' . $this->options;
    }
}
