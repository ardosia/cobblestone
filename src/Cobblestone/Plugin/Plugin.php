<?php

declare(strict_types=1);

namespace Cobblestone\Plugin;

interface Plugin
{
    public function enable(PluginContext $context): void;

    public function disable(): void;
}
