<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Ergonomic semantic legacy block state.
 *
 * Performance-sensitive code carries the scalar BlockStateId token instead; this wrapper is used
 * at API/generator/debug boundaries where named block identity is more important than raw layout.
 */
final readonly class BlockState
{
    public function __construct(
        public BlockType $type,
        public BlockData $data = BlockData::Zero,
    ) {}

    public static function fromId(int $stateId): self
    {
        return BlockStateId::decode($stateId);
    }

    public function stateId(): int
    {
        return BlockStateId::encode($this->type, $this->data);
    }

    public function isAir(): bool
    {
        return $this->type->isAir();
    }
}
