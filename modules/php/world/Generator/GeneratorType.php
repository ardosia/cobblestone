<?php

declare(strict_types=1);

namespace Cobblestone\World\Generator;

enum GeneratorType: int
{
    case Old = 0;
    case Infinite = 1;
    case Flat = 2;
}
