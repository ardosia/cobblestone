<?php

declare(strict_types=1);

namespace Cobblestone\World\Light;

final readonly class BlockLightProperties
{
    public function __construct(
        public int $lightBlock,
        public int $lightEmission,
    ) {
        if ($lightBlock < 0 || $lightBlock > 15 || $lightEmission < 0 || $lightEmission > 15) {
            throw new \ValueError('fixed-target light properties must be in range 0..15');
        }
    }

    public function propagationAttenuation(): int
    {
        return $this->lightBlock === 0 ? 1 : $this->lightBlock;
    }
}
