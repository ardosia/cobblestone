<?php

declare(strict_types=1);

namespace Cobblestone\Tests;

use Cobblestone\Plugin\Plugin;
use Cobblestone\Plugin\PluginScope;
use Cobblestone\Server\Event\ServerStopping;
use function Cobblestone\Command\{greedyString, literal};

final class ServerSmokePlugin implements Plugin
{
    /** @var array<string, mixed> */
    public static array $state = [];

    public function enable(PluginScope $plugin): void
    {
        self::$state['enabled'] = true;
        $plugin->logger()->info('Server smoke plugin enabled');

        $plugin->command(
            literal('smoke')->then(
                literal('echo')->then(
                    greedyString('message')->executes(
                        static fn (string $message): string => $message,
                    ),
                ),
            ),
        );

        $plugin->on(
            ServerStopping::class,
            static function (object $event): void {
                if (!$event instanceof ServerStopping) {
                    throw new \LogicException('unexpected event type');
                }
                self::$state['stopping'] = true;
            },
        );

        $plugin->after(
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
