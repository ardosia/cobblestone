<?php

declare(strict_types=1);

namespace Cobblestone\Tests;

use Cobblestone\Kernel\ServerStopping;
use Cobblestone\Plugin\Plugin;
use Cobblestone\Plugin\PluginContext;

final class KernelSmokePlugin implements Plugin
{
    /** @var array<string, mixed> */
    public static array $state = [];

    public function enable(PluginContext $context): void
    {
        self::$state['enabled'] = true;

        $context->commands->register(
            'smoke:echo',
            static fn (array $arguments): string => implode(':', $arguments),
        );

        $context->events->listen(
            ServerStopping::class,
            static function (object $event): void {
                if (!$event instanceof ServerStopping) {
                    throw new \LogicException('unexpected event type');
                }
                self::$state['stopping'] = true;
            },
        );

        $context->scheduler->schedule(
            1,
            static function (): void {
                self::$state['scheduled'] = true;
            },
        );
    }

    public function disable(): void
    {
        self::$state['disabled'] = true;
    }
}
