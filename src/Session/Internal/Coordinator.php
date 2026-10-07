<?php

declare(strict_types=1);

namespace Cobblestone\Session\Internal;

use Cobblestone\Native\Session as NativeSession;
use Cobblestone\Native\Session\ChunkWork;
use Cobblestone\Native\Session\ChunkWorkResult;
use Cobblestone\Native\Session\ChunkWorkStatus;
use Cobblestone\Native\World\LoadStatus;
use Cobblestone\World\ChunkLoadPending;
use Cobblestone\World\World;
use LogicException;

/**
 * @internal Owner-runtime coordinator for concrete world work requested by native session state.
 *
 * Join/gameplay/view state lives in native/session. PHP only supplies semantic world bootstrap values
 * and advances world generation/storage until the requested chunks are concrete and resident.
 */
final readonly class Coordinator
{
    public function __construct(
        private NativeSession $sessions,
        private World $world,
    ) {}

    public function acceptLogin(int $sessionId): void
    {
        $spawn = $this->world->spawn();
        $this->sessions->acceptLogin(
            $sessionId,
            $this->world->seed(),
            $this->world->generatorType()->value,
            $this->world->dimension(),
            $spawn->x,
            $spawn->y,
            $spawn->z,
            $this->world->time(),
            $this->world->isTimeStarted(),
            $this->world->name(),
        );
    }

    /** @return list<SpawnCompletion> */
    public function tick(): array
    {
        $completed = [];
        $afterSessionId = 0;

        while (($work = $this->sessions->nextChunkWork($afterSessionId)) !== null) {
            $afterSessionId = $work->sessionId;
            if (!$this->prepare($work)) {
                continue;
            }

            $native = $this->world->nativeStore();
            $result = $this->sessions->completeChunkWork($work->sessionId, $native->handle());
            if ($result->status === ChunkWorkStatus::Spawned) {
                $completed[] = $this->spawnCompletion($work, $result);
            }
        }

        return $completed;
    }

    private function prepare(ChunkWork $work): bool
    {
        if ($work->positions === []) {
            return true;
        }

        $native = $this->world->nativeStore();

        $statuses = null;
        if ($native->hasStorage()) {
            $projection = $native::encodeStorageLoadBatch($work->positions);
            $statuses = $native->prepareStorageLoadBatch($projection);
            if (strlen($statuses) !== count($work->positions)) {
                throw new LogicException('session chunk preparation returned the wrong storage status width');
            }
        }

        $complete = true;
        foreach ($work->positions as $index => $position) {
            if ($statuses !== null) {
                $status = LoadStatus::from(ord($statuses[$index]));
                if ($status !== LoadStatus::Resident && $status !== LoadStatus::Missing) {
                    $complete = false;
                    continue;
                }
            }

            try {
                $chunk = $this->world->chunk($position, true);
            } catch (ChunkLoadPending) {
                $complete = false;
                continue;
            }
            if ($chunk === null) {
                throw new LogicException(
                    "world failed to prepare session chunk {$position->x}:{$position->z}",
                );
            }
            $this->sessions->markChunkPrepared($work->sessionId, $native->handle(), $position);
        }

        return $complete;
    }

    private function spawnCompletion(ChunkWork $work, ChunkWorkResult $result): SpawnCompletion
    {
        return new SpawnCompletion(
            $work->sessionId,
            $result->requestedRadius,
            $result->effectiveRadius,
            $result->chunksSent,
            $result->encodedBytes,
            $result->encodeNanos,
        );
    }
}
