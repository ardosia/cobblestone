<?php

declare(strict_types=1);

namespace Cobblestone\World;

enum ChunkUnloadStatus: string
{
    case Missing = 'missing';
    case Pinned = 'pinned';
    case Dirty = 'dirty';
    case Unloaded = 'unloaded';
}
