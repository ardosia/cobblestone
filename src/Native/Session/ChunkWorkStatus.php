<?php

declare(strict_types=1);

namespace Cobblestone\Native\Session;

/** @internal */
enum ChunkWorkStatus: int
{
    case Backpressured = 0;
    case Complete = 1;
    case Spawned = 2;
    case Gone = 3;
}
