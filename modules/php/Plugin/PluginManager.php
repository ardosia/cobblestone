<?php

declare(strict_types=1);

namespace Cobblestone\Plugin;

use LogicException;
use RuntimeException;
use Throwable;

/**
 * Explicit plugin loader for the single owning PHP runtime.
 *
 * C007 intentionally avoids discovery magic. The server supplies an exact PHP file and class name;
 * the class must implement Plugin and receives only PluginContext.
 */
final class PluginManager
{
    /** @var array<class-string<Plugin>, Plugin> */
    private array $plugins = [];

    public function __construct(
        private readonly PluginContext $context,
    ) {
    }

    /**
     * @param class-string<Plugin> $class
     */
    public function load(string $file, string $class): Plugin
    {
        $real = realpath($file);
        if ($real === false || !is_file($real) || strtolower(pathinfo($real, PATHINFO_EXTENSION)) !== 'php') {
            throw new RuntimeException("plugin file is not a readable PHP file: {$file}");
        }
        if (isset($this->plugins[$class])) {
            throw new LogicException("plugin already loaded: {$class}");
        }

        require_once $real;

        if (!class_exists($class)) {
            throw new RuntimeException("plugin class was not defined by {$real}: {$class}");
        }

        $plugin = new $class();
        if (!$plugin instanceof Plugin) {
            throw new RuntimeException("plugin class must implement " . Plugin::class . ": {$class}");
        }

        $plugin->enable($this->context);
        $this->plugins[$class] = $plugin;

        return $plugin;
    }

    public function shutdown(): void
    {
        $firstFailure = null;
        foreach (array_reverse($this->plugins, true) as $plugin) {
            try {
                $plugin->disable();
            } catch (Throwable $error) {
                $firstFailure ??= $error;
            }
        }
        $this->plugins = [];

        if ($firstFailure !== null) {
            throw $firstFailure;
        }
    }
}
