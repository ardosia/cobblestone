<?php

declare(strict_types=1);

namespace Cobblestone\World;

final readonly class LightEditResult
{
    public function __construct(
        public bool $changed,
        public LightRevision $revision,
    ) {
    }
}
