<?php

declare(strict_types=1);

namespace Cobblestone\Event;

use Closure;

final class Subscription
{
    private bool $active = true;

    /** @param Closure(): bool $cancel */
    public function __construct(
        private readonly Closure $cancel,
    ) {
    }

    public function active(): bool
    {
        return $this->active;
    }

    public function cancel(): bool
    {
        if (!$this->active) {
            return false;
        }

        $this->active = false;

        return ($this->cancel)();
    }
}
