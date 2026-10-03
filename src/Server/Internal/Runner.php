<?php

declare(strict_types=1);

namespace Cobblestone\Server\Internal;

use Cobblestone\Server\Server;
use Cobblestone\Server\ServerState;
use Cobblestone\Tick\TickLoop;
use Psr\Log\LoggerInterface;
use Throwable;

/** @internal */
final class Runner
{
    private bool $shutdownHookRegistered = false;

    public function __construct(
        private readonly Server $server,
        private readonly TickLoop $ticks,
        private readonly LoggerInterface $logger,
        private readonly int $nativeEventBudget = 256,
    ) {
        if ($nativeEventBudget <= 0) {
            throw new \InvalidArgumentException('native event budget must be positive');
        }
    }

    public function run(): int
    {
        $this->installSignalHandlers();
        $this->registerShutdownHook();

        $failure = null;
        $tickCount = 0;

        try {
            $this->server->start();
            $tickCount = $this->ticks->run(
                function (int $_tick): void {
                    $this->server->tick($this->nativeEventBudget);
                },
                fn (): bool => !$this->server->isStopRequested(),
            );
        } catch (Throwable $error) {
            $failure = $error;
            $this->logger->critical(
                'Server run failed',
                ['exception' => $error],
            );
            $this->server->stop('server-run-failure');
        } finally {
            try {
                $this->server->shutdown();
            } catch (Throwable $error) {
                if ($failure === null) {
                    $failure = $error;
                } else {
                    $this->logger->error(
                        'Server shutdown also failed',
                        ['exception' => $error],
                    );
                }
            }
        }

        if ($failure !== null) {
            throw $failure;
        }

        return $tickCount;
    }

    private function installSignalHandlers(): void
    {
        if (!function_exists('pcntl_async_signals') || !function_exists('pcntl_signal')) {
            return;
        }

        pcntl_async_signals(true);

        foreach (['SIGINT', 'SIGTERM'] as $signalName) {
            if (!defined($signalName)) {
                continue;
            }

            $signal = constant($signalName);
            pcntl_signal(
                $signal,
                function (int $_signal) use ($signalName): void {
                    $this->server->stop($signalName);
                },
            );
        }
    }

    private function registerShutdownHook(): void
    {
        if ($this->shutdownHookRegistered) {
            return;
        }
        $this->shutdownHookRegistered = true;

        register_shutdown_function(function (): void {
            if ($this->server->state() === ServerState::Stopped) {
                return;
            }

            $this->server->stop('php-shutdown');
            try {
                $this->server->shutdown();
            } catch (Throwable $error) {
                $this->logger->error(
                    'Shutdown hook failed',
                    ['exception' => $error],
                );
            }
        });
    }
}
