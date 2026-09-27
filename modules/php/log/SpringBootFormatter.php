<?php

declare(strict_types=1);

namespace Cobblestone\Log;

use Monolog\Formatter\FormatterInterface;
use Monolog\Level;
use Monolog\LogRecord;
use Stringable;
use Throwable;

final class SpringBootFormatter implements FormatterInterface
{
    public function format(LogRecord $record): string
    {
        $context = $record->context;
        $logger = (string) ($context['__logger'] ?? $record->channel);
        $execution = (string) ($context['__execution'] ?? 'main');
        unset($context['__logger'], $context['__execution']);

        $message = $this->interpolate($record->message, $context);
        $suffix = $this->contextSuffix($context);
        $exception = $record->context['exception'] ?? null;

        $line = sprintf(
            "%s %5s %d --- [%15s] %-40s : %s%s\n",
            $record->datetime->format('Y-m-d\TH:i:s.vP'),
            $this->levelName($record->level),
            getmypid() ?: 0,
            $this->tail($execution, 15),
            $this->tail($logger, 40),
            $message,
            $suffix,
        );

        if ($exception instanceof Throwable) {
            $line .= sprintf(
                "%s: %s\n%s\n",
                $exception::class,
                $exception->getMessage(),
                $exception->getTraceAsString(),
            );
        }

        return $line;
    }

    /** @param list<LogRecord> $records
     *  @return list<string>
     */
    public function formatBatch(array $records): array
    {
        return array_map($this->format(...), $records);
    }

    private function levelName(Level $level): string
    {
        return match (true) {
            $level->value >= Level::Error->value => 'ERROR',
            $level->value >= Level::Warning->value => 'WARN',
            $level->value >= Level::Info->value => 'INFO',
            default => 'DEBUG',
        };
    }

    /** @param array<string, mixed> $context */
    private function interpolate(string $message, array &$context): string
    {
        $replace = [];
        foreach ($context as $key => $value) {
            if (
                is_scalar($value)
                || $value === null
                || $value instanceof Stringable
            ) {
                $replace['{' . $key . '}'] = (string) $value;
            }
        }

        if ($replace === []) {
            return $message;
        }

        $interpolated = strtr($message, $replace);
        foreach ($replace as $placeholder => $_) {
            if (str_contains($message, $placeholder)) {
                unset($context[substr($placeholder, 1, -1)]);
            }
        }

        return $interpolated;
    }

    /** @param array<string, mixed> $context */
    private function contextSuffix(array $context): string
    {
        unset($context['exception']);
        if ($context === []) {
            return '';
        }

        ksort($context);
        $pairs = [];
        foreach ($context as $key => $value) {
            $pairs[] = $key . '=' . $this->render($value);
        }

        return ' ' . implode(' ', $pairs);
    }

    private function render(mixed $value): string
    {
        if ($value === null) {
            return 'null';
        }
        if (is_bool($value)) {
            return $value ? 'true' : 'false';
        }
        if (is_int($value) || is_float($value)) {
            return (string) $value;
        }
        if (is_string($value) || $value instanceof Stringable) {
            return (string) json_encode(
                (string) $value,
                JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_INVALID_UTF8_SUBSTITUTE,
            );
        }

        $encoded = json_encode(
            $value,
            JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_INVALID_UTF8_SUBSTITUTE,
        );

        return $encoded === false ? get_debug_type($value) : $encoded;
    }

    private function tail(string $value, int $width): string
    {
        if (strlen($value) <= $width) {
            return $value;
        }

        return substr($value, -$width);
    }
}
