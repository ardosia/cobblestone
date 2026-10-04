<?php

declare(strict_types=1);

namespace Cobblestone\World;

use ValueError;

/**
 * Compact fixed-target state token: (legacy block id << 4) | data.
 *
 * This is intentionally scalar currency for chunk/native/storage/protocol hot paths. Semantic code
 * should construct BlockState from BlockType + BlockData and cross this codec only at boundaries.
 */
final class BlockStateId
{
    public const MAX = 0x0fff;

    private function __construct()
    {
    }

    public static function encode(BlockType $type, BlockData $data = BlockData::Zero): int
    {
        return ($type->value << 4) | $data->value;
    }

    public static function decode(int $stateId): BlockState
    {
        self::assert($stateId);

        return new BlockState(
            self::typeUnchecked($stateId),
            BlockData::from($stateId & 0x0f),
        );
    }

    public static function assert(int $stateId): int
    {
        if ($stateId < 0 || $stateId > self::MAX) {
            throw new ValueError('fixed-target block state id must be in range 0..4095');
        }

        if (BlockType::tryFrom($stateId >> 4) === null) {
            throw new ValueError('unsupported MCPE 0.15.10 block id ' . ($stateId >> 4));
        }

        return $stateId;
    }

    public static function type(int $stateId): BlockType
    {
        self::assert($stateId);

        return self::typeUnchecked($stateId);
    }

    public static function data(int $stateId): BlockData
    {
        self::assert($stateId);

        return BlockData::from($stateId & 0x0f);
    }

    public static function isAir(int $stateId): bool
    {
        return self::type($stateId) === BlockType::Air;
    }

    private static function typeUnchecked(int $stateId): BlockType
    {
        return BlockType::from($stateId >> 4);
    }
}
