<?php

declare(strict_types=1);

namespace Cobblestone\Server;

enum ServerState: string
{
    case Starting = 'starting';
    case Running = 'running';
    case Stopping = 'stopping';
    case Stopped = 'stopped';
}
