<?php

declare(strict_types=1);

namespace Cobblestone\Log;

use DateTimeZone;
use InvalidArgumentException;
use Monolog\Handler\StreamHandler;
use Monolog\Level;
use Monolog\Logger;
use Psr\Log\LoggerInterface;

final class LoggerFactory
{
    private function __construct(
        private readonly LoggerInterface $root,
    ) {}

    public static function console(
        ?string $minimumLevel = null,
        string $applicationName = 'Cobblestone',
    ): self {
        $level = self::normalizeLevel($minimumLevel ?? 'INFO');
        $handler = new StreamHandler('php://stdout', Level::fromName($level), true);
        $handler->setFormatter(new ConsoleFormatter($applicationName));

        $logger = new Logger($applicationName, [$handler]);
        $logger->setTimezone(new DateTimeZone(date_default_timezone_get()));

        return new self($logger);
    }

    /**
     * @param array<string, mixed> $context
     */
    public function logger(
        string $name,
        array $context = [],
        string $execution = 'main',
    ): LoggerInterface {
        $name = trim($name);
        if ($name === '') {
            throw new InvalidArgumentException('logger name cannot be empty');
        }
        if ($execution === '') {
            throw new InvalidArgumentException('logger execution label cannot be empty');
        }

        return new ContextLogger($this->root, $name, $context, $execution);
    }

    private static function normalizeLevel(string $level): string
    {
        return match (strtoupper(trim($level))) {
            'TRACE', 'DEBUG' => 'DEBUG',
            'INFO' => 'INFO',
            'NOTICE' => 'NOTICE',
            'WARN', 'WARNING' => 'WARNING',
            'ERROR' => 'ERROR',
            'CRITICAL' => 'CRITICAL',
            'ALERT' => 'ALERT',
            'EMERGENCY' => 'EMERGENCY',
            default => throw new InvalidArgumentException("unknown log level {$level}"),
        };
    }
}
