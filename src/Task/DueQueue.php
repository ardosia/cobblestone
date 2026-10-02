<?php

declare(strict_types=1);

namespace Cobblestone\Task;

/**
 * @internal
 *
 * Stable binary min-heap ordered by due tick and then identity.
 *
 * Entries may become stale when the scheduler cancels work. Callers validate popped entries
 * against their authoritative maps, keeping cancellation O(1) without reintroducing full scans.
 */
final class DueQueue
{
    /** @var list<array{due: int, id: int}> */
    private array $heap = [];

    public function push(int $due, int $id): void
    {
        $index = count($this->heap);
        $this->heap[] = ['due' => $due, 'id' => $id];

        while ($index > 0) {
            $parent = intdiv($index - 1, 2);
            if (!self::before($this->heap[$index], $this->heap[$parent])) {
                break;
            }

            [$this->heap[$parent], $this->heap[$index]] = [$this->heap[$index], $this->heap[$parent]];
            $index = $parent;
        }
    }

    public function peekDue(): ?int
    {
        return $this->heap[0]['due'] ?? null;
    }

    /** @return array{due: int, id: int}|null */
    public function pop(): ?array
    {
        if ($this->heap === []) {
            return null;
        }

        $root = $this->heap[0];
        $last = array_pop($this->heap);
        if ($this->heap === []) {
            return $root;
        }

        $this->heap[0] = $last;
        $count = count($this->heap);
        $index = 0;

        while (true) {
            $left = ($index * 2) + 1;
            if ($left >= $count) {
                break;
            }

            $right = $left + 1;
            $smallest = $left;
            if ($right < $count && self::before($this->heap[$right], $this->heap[$left])) {
                $smallest = $right;
            }
            if (!self::before($this->heap[$smallest], $this->heap[$index])) {
                break;
            }

            [$this->heap[$index], $this->heap[$smallest]] = [$this->heap[$smallest], $this->heap[$index]];
            $index = $smallest;
        }

        return $root;
    }

    public function clear(): void
    {
        $this->heap = [];
    }

    /**
     * @param array{due: int, id: int} $left
     * @param array{due: int, id: int} $right
     */
    private static function before(array $left, array $right): bool
    {
        return $left['due'] < $right['due']
            || ($left['due'] === $right['due'] && $left['id'] < $right['id']);
    }
}
