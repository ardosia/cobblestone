<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Semantic fixed-target block identity.
 *
 * Block behavior remains out of scope; this value only exposes registry identity and static light
 * metadata while BlockState carries the legacy 4-bit data value.
 */
final readonly class BlockType
{
    public string $name;
    public int $lightBlock;
    public int $lightEmission;

    public function __construct(public int $id)
    {
        $this->name = BlockCatalog::name($id);
        $this->lightBlock = BlockCatalog::lightBlock($id);
        $this->lightEmission = BlockCatalog::lightEmission($id);
    }

    public static function fromName(string $name): self
    {
        return new self(BlockCatalog::idForName($name));
    }

    public function state(int $data = 0): BlockState
    {
        return new BlockState($this->id, $data);
    }

    public function isAir(): bool
    {
        return $this->id === 0;
    }
}
