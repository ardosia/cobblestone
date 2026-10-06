<?php

declare(strict_types=1);

namespace Cobblestone\Config;

use InvalidArgumentException;

final readonly class StorageConfig
{
    public const DEFAULT_COMPACTION_MIN_DEAD_BYTES = 64 * 1024 * 1024;
    public const DEFAULT_COMPACTION_MIN_DEAD_PERCENT = 50;

    public function __construct(
        public int $saveWorkers = 2,
        public int $loadWorkers = 2,
        public int $compactionMinDeadBytes = self::DEFAULT_COMPACTION_MIN_DEAD_BYTES,
        public int $compactionMinDeadPercent = self::DEFAULT_COMPACTION_MIN_DEAD_PERCENT,
    ) {
        if ($saveWorkers < 1 || $saveWorkers > 32) {
            throw new InvalidArgumentException('save worker count must be in range 1..32');
        }
        if ($loadWorkers < 1 || $loadWorkers > 32) {
            throw new InvalidArgumentException('load worker count must be in range 1..32');
        }
        if ($compactionMinDeadBytes < 0) {
            throw new InvalidArgumentException('compaction minimum dead bytes must be nonnegative');
        }
        if ($compactionMinDeadPercent < 0 || $compactionMinDeadPercent > 100) {
            throw new InvalidArgumentException('compaction minimum dead percent must be in range 0..100');
        }
    }
}
