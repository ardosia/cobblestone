<?php

declare(strict_types=1);

namespace Cobblestone\Command;

class CommandParseException extends \RuntimeException
{
    public function __construct(string $message, public readonly int $cursor = 0)
    {
        parent::__construct($message);
    }
}
