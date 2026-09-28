<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * PHP-backed chunk revision, lifecycle, and persistence-watermark state.
 *
 * @internal
 */
final class ChunkFallbackState
{
    private bool $generated = false;
    private bool $populated = false;
    private bool $lightPopulated = false;
    private int $revision = 0;
    private int $lightRevision = 0;
    private ?int $persistedRevision = null;
    private ?int $persistedLightRevision = null;
    private ?int $persistedLifecycleFlags = null;

    /** @var array<int, int> */
    private array $extraData = [];

    public function revision(): int
    {
        return $this->revision;
    }

    public function lightRevision(): int
    {
        return $this->lightRevision;
    }

    public function commitRevision(int $expected, int $next): void
    {
        if ($this->revision !== $expected) {
            throw new \LogicException(
                "chunk revision changed: expected {$expected}, current {$this->revision}",
            );
        }
        if ($next !== $expected + 1) {
            throw new \LogicException('chunk mutation revision must advance exactly once');
        }

        $this->revision = $next;
    }
    public function commitLightRevision(int $expected, int $next): void
    {
        if ($this->lightRevision !== $expected) {
            throw new \LogicException(
                "chunk light revision changed: expected {$expected}, current {$this->lightRevision}",
            );
        }
        if ($next !== $expected + 1) {
            throw new \LogicException('chunk light revision must advance exactly once');
        }

        $this->lightRevision = $next;
    }

    public function lifecycleFlags(): int
    {
        return ($this->generated ? Chunk::LIFECYCLE_GENERATED : 0)
            | ($this->populated ? Chunk::LIFECYCLE_POPULATED : 0)
            | ($this->lightPopulated ? Chunk::LIFECYCLE_LIGHT_POPULATED : 0);
    }

    public function setLifecycleFlags(int $flags): void
    {
        $this->generated = ($flags & Chunk::LIFECYCLE_GENERATED) !== 0;
        $this->populated = ($flags & Chunk::LIFECYCLE_POPULATED) !== 0;
        $this->lightPopulated = ($flags & Chunk::LIFECYCLE_LIGHT_POPULATED) !== 0;
    }

    public function blockExtraData(int $key): int
    {
        return $this->extraData[$key] ?? 0;
    }

    public function setBlockExtraData(int $key, int $data): int
    {
        $previous = $this->extraData[$key] ?? 0;
        if ($data === 0) {
            unset($this->extraData[$key]);
        } else {
            $this->extraData[$key] = $data;
        }

        return $previous;
    }

    /** @return array<int, int> */
    public function extraData(): array
    {
        return $this->extraData;
    }

    public function isDirty(): bool
    {
        return $this->persistedRevision !== $this->revision
            || $this->persistedLightRevision !== $this->lightRevision
            || $this->persistedLifecycleFlags !== $this->lifecycleFlags();
    }

    public function markPersisted(
        int $terrainRevision,
        int $lightRevision,
        int $lifecycleFlags,
    ): void {
        if ($terrainRevision > $this->revision || $lightRevision > $this->lightRevision) {
            throw new \LogicException('persisted chunk revision cannot exceed live revision');
        }
        if ($this->persistedRevision !== null && $terrainRevision < $this->persistedRevision) {
            throw new \LogicException('persisted terrain revision cannot regress');
        }
        if (
            $this->persistedLightRevision !== null
            && $lightRevision < $this->persistedLightRevision
        ) {
            throw new \LogicException('persisted light revision cannot regress');
        }

        $this->persistedRevision = $terrainRevision;
        $this->persistedLightRevision = $lightRevision;
        $this->persistedLifecycleFlags = $lifecycleFlags;
    }
}
