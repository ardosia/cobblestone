<?php

declare(strict_types=1);

namespace Cobblestone\Native\Session;

enum ViewSendResult: int
{
    case Backpressured = 0;
    case Sent = 1;
    case Gone = 2;
}
