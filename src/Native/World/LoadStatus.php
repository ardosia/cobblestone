<?php

declare(strict_types=1);

namespace Cobblestone\Native\World;

/** @internal Native durable chunk-load state. */
enum LoadStatus: int
{
    case Resident = 0;
    case Queued = 1;
    case Joined = 2;
    case Missing = 3;
}
