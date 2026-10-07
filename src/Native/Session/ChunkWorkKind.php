<?php

declare(strict_types=1);

namespace Cobblestone\Native\Session;

/** @internal */
enum ChunkWorkKind: int
{
    case Initial = 0;
    case View = 1;
}
