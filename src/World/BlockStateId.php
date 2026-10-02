<?php

declare(strict_types=1);

namespace Cobblestone\World;

use ValueError;

/**
 * Fixed-target legacy state token: (block id << 4) | data.
 *
 * Hot paths use this scalar directly; BlockState remains an ergonomic boundary wrapper.
 */
final class BlockStateId
{
    public const MAX = 0x0fff;

    private function __construct()
    {
    }

    public static function fromLegacy(int $id, int $data = 0): int
    {
        if ($id < 0 || $id > 0xff) {
            throw new ValueError('fixed-target block id must be in range 0..255');
        }
        if ($data < 0 || $data > 0x0f) {
            throw new ValueError('fixed-target block data must be in range 0..15');
        }

        return ($id << 4) | $data;
    }

    public static function assert(int $stateId): int
    {
        if ($stateId < 0 || $stateId > self::MAX) {
            throw new ValueError('fixed-target block state id must be in range 0..4095');
        }

        return $stateId;
    }

    public static function blockId(int $stateId): int
    {
        return self::assert($stateId) >> 4;
    }

    public static function data(int $stateId): int
    {
        return self::assert($stateId) & 0x0f;
    }

    public static function isAir(int $stateId): bool
    {
        return self::blockId($stateId) === 0;
    }
}
