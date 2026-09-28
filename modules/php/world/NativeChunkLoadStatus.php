<?php

declare(strict_types=1);

namespace Cobblestone\World;

/** @internal Native durable chunk-load state. */
enum NativeChunkLoadStatus: int
{
    case Resident = 0;
    case Queued = 1;
    case Joined = 2;
    case Missing = 3;
}
