<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Ergonomic legacy-state wrapper.
 *
 * Performance-sensitive code should carry the scalar BlockStateId token instead.
 */
final readonly class BlockState
{
    public function __construct(
        public int $id,
        public int $data = 0,
    ) {
        BlockStateId::fromLegacy($id, $data);
    }

    public static function air(): self
    {
        return new self(0);
    }

    public static function fromId(int $stateId): self
    {
        BlockStateId::assert($stateId);

        return new self($stateId >> 4, $stateId & 0x0f);
    }

    public function type(): BlockType
    {
        return BlockCatalog::type($this->id);
    }

    public function fullId(): int
    {
        return BlockStateId::fromLegacy($this->id, $this->data);
    }

    public function isAir(): bool
    {
        return $this->id === 0;
    }
}
