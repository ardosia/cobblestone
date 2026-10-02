<?php

declare(strict_types=1);

namespace Cobblestone\Log;

use Psr\Log\AbstractLogger;
use Psr\Log\LoggerInterface;
use Stringable;

final class ContextLogger extends AbstractLogger
{
    /** @param array<string, mixed> $context */
    public function __construct(
        private readonly LoggerInterface $logger,
        private readonly string $name,
        private readonly array $context = [],
        private readonly string $execution = 'main',
    ) {
    }

    public function log($level, string|Stringable $message, array $context = []): void
    {
        $this->logger->log(
            $level,
            $message,
            array_merge(
                $this->context,
                $context,
                [
                    '__logger' => $this->name,
                    '__execution' => $this->execution,
                ],
            ),
        );
    }
}
