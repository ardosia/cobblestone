<?php

declare(strict_types=1);

namespace Cobblestone\Plugin\Internal;

use Cobblestone\Command\Internal\CommandTree;
use Cobblestone\Event\Internal\Dispatcher;
use Cobblestone\Log\LoggerFactory;
use Cobblestone\Plugin\Plugin;
use Cobblestone\Plugin\PluginScope;
use Cobblestone\Task\Scheduler;
use LogicException;
use Psr\Log\LoggerInterface;
use RuntimeException;
use Throwable;

/**
 * Explicit plugin loader for the owning PHP gameplay runtime.
 *
 * Discovery magic stays out of the runtime. Every plugin receives an owned PluginScope; runtime and
 * thread ownership machinery remains internal.
 */
/** @internal */
final class Plugins
{
    /** @var array<class-string<Plugin>, array{plugin: Plugin, scope: PluginScope}> */
    private array $plugins = [];

    private readonly LoggerInterface $logger;

    public function __construct(
        private readonly Dispatcher $events,
        private readonly CommandTree $commands,
        private readonly Scheduler $scheduler,
        private readonly LoggerFactory $logs,
    ) {
        $this->logger = $logs->logger('Cobblestone.Plugin');
    }

    /** @param class-string<Plugin> $class */
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

        $scope = new PluginScope(
            $this->events,
            $this->commands,
            $this->scheduler,
            $this->logs->logger(
                'Cobblestone.Plugin.' . str_replace('\\', '.', $class),
                ['plugin' => $class],
            ),
        );

        try {
            $plugin->enable($scope);
        } catch (Throwable $error) {
            try {
                $scope->close();
            } catch (Throwable $cleanupError) {
                $this->logger->error(
                    'Plugin enable rollback failed',
                    ['plugin' => $class, 'exception' => $cleanupError],
                );
            }
            throw $error;
        }

        $this->plugins[$class] = ['plugin' => $plugin, 'scope' => $scope];
        $this->logger->info('Enabled plugin', ['plugin' => $class]);

        return $plugin;
    }

    /** @param class-string<Plugin> $class */
    public function unload(string $class): bool
    {
        $entry = $this->plugins[$class] ?? null;
        if ($entry === null) {
            return false;
        }
        unset($this->plugins[$class]);

        $this->disable($class, $entry['plugin'], $entry['scope']);

        return true;
    }

    public function shutdown(): void
    {
        $failure = null;
        foreach (array_reverse(array_keys($this->plugins)) as $class) {
            try {
                $this->unload($class);
            } catch (Throwable $error) {
                $failure ??= $error;
            }
        }

        if ($failure !== null) {
            throw $failure;
        }
    }

    /** @param class-string<Plugin> $class */
    private function disable(string $class, Plugin $plugin, PluginScope $scope): void
    {
        $failure = null;
        try {
            $plugin->disable();
        } catch (Throwable $error) {
            $failure = $error;
            $this->logger->error(
                'Plugin disable failed',
                ['plugin' => $class, 'exception' => $error],
            );
        }

        try {
            $scope->close();
        } catch (Throwable $error) {
            $failure ??= $error;
            $this->logger->error(
                'Plugin scope cleanup failed',
                ['plugin' => $class, 'exception' => $error],
            );
        }

        if ($failure !== null) {
            throw $failure;
        }

        $this->logger->info('Disabled plugin', ['plugin' => $class]);
    }
}
