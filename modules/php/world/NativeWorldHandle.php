<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Native world-store handle lifecycle.
 *
 * @internal
 */
final class NativeWorldHandle
{
    private function __construct()
    {
    }

    public static function create(): int
    {
        return cobblestone_world_create();
    }

    public static function destroy(int $handle): void
    {
        cobblestone_world_destroy($handle);
    }

    public static function destroyIfAvailable(int $handle): void
    {
        if (!\function_exists('cobblestone_world_destroy')) {
            return;
        }

        try {
            self::destroy($handle);
        } catch (\Throwable) {
            // Extension/module shutdown owns the final cleanup fallback.
        }
    }
}
