<?php

declare(strict_types=1);

namespace Cobblestone\Tick;

use Closure;
use Psr\Log\LoggerInterface;

final class TickLoop
{
    private readonly Clock $clock;

    public function __construct(
        private readonly TickLoopConfig $config,
        private readonly LoggerInterface $logger,
        ?Clock $clock = null,
    ) {
        $this->clock = $clock ?? new SystemClock();
    }

    /**
     * @param Closure(int): void $tick
     * @param Closure(): bool $keepRunning
     */
    public function run(Closure $tick, Closure $keepRunning): int
    {
        $period = $this->config->periodNanos();
        $nextDeadline = $this->clock->nowNanos();
        $lastWarningAt = null;
        $tickNumber = 0;

        $warnIfBehind = function (int $now, int $behind) use (&$lastWarningAt, $period): void {
            if ($behind <= 0 || $behind < $this->config->warningThresholdNanos()) {
                return;
            }
            if (
                $lastWarningAt !== null
                && ($now - $lastWarningAt) < $this->config->warningIntervalNanos()
            ) {
                return;
            }

            $lastWarningAt = $now;
            $this->logger->warning(
                "Can't keep up! Is the server overloaded? Running {behind_ms}ms or {ticks_behind} ticks behind",
                [
                    'behind_ms' => round($behind / 1_000_000, 3),
                    'ticks_behind' => max(1, intdiv($behind, $period)),
                ],
            );
        };

        while ($keepRunning()) {
            $now = $this->clock->nowNanos();
            if ($now < $nextDeadline) {
                $this->clock->sleepNanos($nextDeadline - $now);
                $now = $this->clock->nowNanos();
            }

            if (!$keepRunning()) {
                break;
            }

            $warnIfBehind($now, max(0, $now - $nextDeadline));

            ++$tickNumber;
            $tick($tickNumber);

            $finished = $this->clock->nowNanos();
            $nextDeadline += $period;
            $postTickBehind = max(0, $finished - $nextDeadline);
            $warnIfBehind($finished, $postTickBehind);

            $maxBacklog = max(
                $period * $this->config->maxCatchUpTicks,
                $this->config->warningThresholdNanos(),
            );
            if ($postTickBehind > $maxBacklog) {
                $this->logger->debug(
                    'Rebased tick deadline after excessive backlog',
                    [
                        'behind_ms' => round($postTickBehind / 1_000_000, 3),
                        'max_catch_up_ticks' => $this->config->maxCatchUpTicks,
                    ],
                );
                $nextDeadline = $finished + $period;
            }
        }

        return $tickNumber;
    }
}