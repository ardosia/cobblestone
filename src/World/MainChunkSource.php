<?php

declare(strict_types=1);

namespace Cobblestone\World;

use Cobblestone\Native\World as NativeWorld;
use Cobblestone\Native\World\LoadStatus;

use Cobblestone\World\Generator\Generator;
use Cobblestone\World\Generator\InfiniteGenerator;

final class MainChunkSource
{
    /** @var array<string, Chunk> */
    private array $chunks = [];

    /** @var array<string, ResidentChunkCell> */
    private array $cells = [];

    /** @var array<string, true> */
    private array $loading = [];

    /** @var array<string, true> */
    private array $evictionQueued = [];

    /** @var \SplQueue<string> */
    private readonly \SplQueue $evictionQueue;

    public function __construct(
        private readonly Generator $generator,
        private readonly int $seed,
        private readonly ?NativeWorld $nativeStore = null,
    ) {
        $this->evictionQueue = new \SplQueue();
    }

    public function get(ChunkPos $position): ?Chunk
    {
        return $this->chunks[$position->key()] ?? null;
    }

    public function getOrGenerate(ChunkPos $position): Chunk
    {
        $existing = $this->get($position);
        if ($existing !== null) {
            return $this->completeInfiniteChunk($existing);
        }

        if ($this->nativeStore !== null && $this->nativeStore->hasStorage()) {
            $status = $this->nativeStore->requestStorageLoad($position);
            if ($status === LoadStatus::Resident) {
                return $this->completeInfiniteChunk($this->adoptNativeResident($position));
            }
            if ($status !== LoadStatus::Missing) {
                throw new ChunkLoadPending($position, $status);
            }
        }

        $key = $position->key();
        if (isset($this->loading[$key])) {
            throw new \LogicException("chunk {$key} is already being loaded or generated");
        }

        $this->loading[$key] = true;
        try {
            $chunk = $this->generator->generate($position, $this->seed, $this->nativeStore);
            if ($chunk->position()->x !== $position->x || $chunk->position()->z !== $position->z) {
                throw new \LogicException('world generator returned a chunk for the wrong position');
            }

            $chunk->markGenerated();
            $this->generator->populate($chunk, $this->seed);
            $chunk->markPopulated();
            $this->put($chunk);

            return $chunk;
        } finally {
            unset($this->loading[$key]);
        }
    }

    private function completeInfiniteChunk(Chunk $chunk): Chunk
    {
        // The native 3x3 population view persists generated-only neighbors. A loaded chunk
        // is not ready to serve until its own center post-process and lighting have run.
        if ($this->generator instanceof InfiniteGenerator
            && (!$chunk->isPopulated() || !$chunk->isLightPopulated())
        ) {
            $this->generator->generate($chunk->position(), $this->seed, $this->nativeStore);
        }

        return $chunk;
    }

    public function adoptNativeResident(ChunkPos $position): Chunk
    {
        $existing = $this->get($position);
        if ($existing !== null) {
            return $existing;
        }
        if ($this->nativeStore === null) {
            throw new \LogicException('cannot adopt native residency without a native world store');
        }

        $chunk = new Chunk(
            $position,
            nativeStore: $this->nativeStore,
            nativeResident: true,
        );
        $this->put($chunk);

        return $chunk;
    }

    /** @internal */
    public function nativeStore(): ?NativeWorld
    {
        return $this->nativeStore;
    }

    public function put(Chunk $chunk): void
    {
        if (!$chunk->matchesNativeStore($this->nativeStore)) {
            throw new \LogicException('chunk source/store mismatch');
        }

        $key = $chunk->position()->key();
        $existing = $this->chunks[$key] ?? null;
        if ($existing !== null && $existing !== $chunk) {
            throw new \LogicException("chunk {$key} already has a different resident object");
        }

        $this->chunks[$key] = $chunk;
        $this->cells[$key] ??= new ResidentChunkCell($chunk);

        if ($existing === null && !isset($this->evictionQueued[$key])) {
            $this->evictionQueued[$key] = true;
            $this->evictionQueue->enqueue($key);
        }
    }

    public function unload(ChunkPos $position): ChunkUnloadStatus
    {
        $key = $position->key();
        $chunk = $this->chunks[$key] ?? null;
        if ($chunk === null) {
            return ChunkUnloadStatus::Missing;
        }

        $cell = $this->cells[$key]
            ?? throw new \LogicException('resident chunk cell missing for resident chunk');
        $status = $chunk->tryEvictBacking($cell->pinCount());

        if ($status === ChunkUnloadStatus::Unloaded || $status === ChunkUnloadStatus::Missing) {
            unset($this->chunks[$key], $this->cells[$key]);
        }

        return $status;
    }

    public function evictCleanUnpinned(int $budget): int
    {
        if ($budget <= 0 || $budget > 4096) {
            throw new \ValueError('chunk eviction budget must be in range 1..4096');
        }

        $removed = 0;
        for ($inspected = 0; $inspected < $budget && !$this->evictionQueue->isEmpty(); ++$inspected) {
            $key = $this->evictionQueue->dequeue();
            unset($this->evictionQueued[$key]);

            $chunk = $this->chunks[$key] ?? null;
            if ($chunk === null) {
                continue;
            }

            $status = $this->unload($chunk->position());
            if ($status === ChunkUnloadStatus::Pinned || $status === ChunkUnloadStatus::Dirty) {
                $this->evictionQueued[$key] = true;
                $this->evictionQueue->enqueue($key);
                continue;
            }

            ++$removed;
        }

        return $removed;
    }

    public function count(): int
    {
        return count($this->chunks);
    }

    public function resident(ChunkPos $position, bool $generate = false): ?ChunkLease
    {
        $chunk = $generate ? $this->getOrGenerate($position) : $this->get($position);
        if ($chunk === null) {
            return null;
        }

        $key = $position->key();
        $cell = $this->cells[$key] ??= new ResidentChunkCell($chunk);

        return new ChunkLease($cell);
    }
}
