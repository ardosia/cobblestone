<?php

declare(strict_types=1);

namespace Cobblestone\Task;

use Closure;

final class TaskHandle
{
    private bool $cancelled = false;

    /**
     * @param Closure(): bool $cancel
     * @param Closure(): bool $active
     */
    public function __construct(
        private readonly Closure $cancel,
        private readonly Closure $active,
    ) {}

    public function active(): bool
    {
        return !$this->cancelled && ($this->active)();
    }

    public function cancel(): bool
    {
        if ($this->cancelled) {
            return false;
        }

        $this->cancelled = true;

        return ($this->cancel)();
    }
}
