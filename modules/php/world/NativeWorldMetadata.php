<?php

declare(strict_types=1);

namespace Cobblestone\World;

/** @internal Authoritative world.cwm projection returned once when native storage is attached. */
final readonly class NativeWorldMetadata
{
    public function __construct(
        public bool $created,
        public string $uuid,
        public string $name,
        public int $seed,
        public int $generatorId,
        public int $generatorSettingsVersion,
        public string $generatorSettings,
        public BlockPos $spawn,
        public int $time,
        public bool $timeRunning,
        public int $generation,
    ) {
        if (strlen($uuid) !== 16) {
            throw new \ValueError('native world metadata UUID must contain exactly 16 bytes');
        }
        if ($name === '') {
            throw new \ValueError('native world metadata name cannot be empty');
        }
        if ($generation <= 0) {
            throw new \ValueError('native world metadata generation must be positive');
        }
    }

    /** @param array<int, mixed> $values */
    public static function fromNative(array $values): self
    {
        if (count($values) !== 13) {
            throw new \UnexpectedValueException('native world metadata projection has wrong width');
        }

        return new self(
            (bool) $values[0],
            (string) $values[1],
            (string) $values[2],
            (int) $values[3],
            (int) $values[4],
            (int) $values[5],
            (string) $values[6],
            new BlockPos((int) $values[7], (int) $values[8], (int) $values[9]),
            (int) $values[10],
            (bool) $values[11],
            (int) $values[12],
        );
    }
}
