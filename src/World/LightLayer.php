<?php

declare(strict_types=1);

namespace Cobblestone\World;

/** The two fixed-target stored-light channels. */
enum LightLayer
{
    case Sky;
    case Block;
}
