<?php

declare(strict_types=1);

namespace Cobblestone\Server;

use Cobblestone\Native\Session\Runtime;
use Cobblestone\World\World;
use Psr\Log\LoggerInterface;

/** @internal */
final class WorldMaintenance
{
    private const CHUNK_EVICTION_BUDGET = 64;

    public function __construct(
        private readonly Runtime $sessions,
        private readonly World $world,
        private readonly LoggerInterface $logger,
    ) {
    }

    public function tickStorage(): void
    {
        $nativeStore = $this->world->nativeStore();
        if ($nativeStore === null || !$nativeStore->hasStorage()) {
            return;
        }

        $storageTick = $nativeStore->storageTick(64);
        if ($storageTick['compaction_failed'] === 0) {
            return;
        }

        $storageStats = $nativeStore->storageStats();
        $this->logger->warning(
            'Persistent region compaction failed; automatic retry is blocked for this process',
            [
                'failures' => $storageTick['compaction_failed'],
                'blocked_regions' => $storageStats['compaction_blocked_regions'],
                'last_error' => $storageStats['compaction_last_error'],
            ],
        );
    }

    public function flushWorldChanges(): void
    {
        $nativeStore = $this->world->nativeStore();
        if ($nativeStore === null) {
            return;
        }

        $this->sessions->flushWorldChanges($nativeStore->handle());
        if ($nativeStore->hasStorage()) {
            $this->world->chunks()->evictCleanUnpinned(self::CHUNK_EVICTION_BUDGET);
        }
    }
}
