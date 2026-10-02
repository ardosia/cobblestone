<?php

declare(strict_types=1);

namespace Cobblestone\Plugin;

interface Plugin
{
    public function enable(PluginScope $plugin): void;

    public function disable(): void;
}
