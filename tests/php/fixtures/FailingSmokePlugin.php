<?php

declare(strict_types=1);

namespace Cobblestone\Tests;

use Cobblestone\Plugin\Plugin;
use Cobblestone\Plugin\PluginScope;
use RuntimeException;
use function Cobblestone\Command\literal;

final class FailingSmokePlugin implements Plugin
{
    /** @var array<string, mixed> */
    public static array $state = [];

    public function enable(PluginScope $plugin): void
    {
        $plugin->command(
            literal('smoke')->then(
                literal('rollback')->executes(
                    static function (): string {
                        self::$state['command'] = true;

                        return 'bad';
                    },
                ),
            ),
        );

        $plugin->after(
            1,
            static function (): void {
                self::$state['task'] = true;
            },
        );

        throw new RuntimeException('expected plugin enable failure');
    }

    public function disable(): void
    {
        self::$state['disabled'] = true;
    }
}
