<?php

declare(strict_types=1);

namespace Cobblestone\Plugin;

use Cobblestone\Command\CommandRegistry;
use Cobblestone\Event\EventBus;
use Cobblestone\Log\LoggerFactory;
use Cobblestone\Task\Scheduler;
use LogicException;
use Psr\Log\LoggerInterface;
use RuntimeException;
use Throwable;

/**
 * Explicit plugin loader for the owning PHP gameplay runtime.
 *
 * Discovery magic stays out of the runtime. Every plugin receives semantic services and a
 * structured logger; runtime/thread ownership machinery remains internal.
 */
final class PluginManager
{
    /** @var array<class-string<Plugin>, Plugin> */
    private array $plugins = [];

    private readonly LoggerInterface $logger;

    public function __construct(
        private readonly EventBus $events,
        private readonly CommandRegistry $commands,
        private readonly Scheduler $scheduler,
        private readonly LoggerFactory $logs,
    ) {
        $this->logger = $logs->logger('Cobblestone.Plugin');
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

        $context = new PluginContext(
            $this->events,
            $this->commands,
            $this->scheduler,
            $this->logs->logger(
                'Cobblestone.Plugin.' . str_replace('\\', '.', $class),
                ['plugin' => $class],
            ),
        );

        $plugin->enable($context);
        $this->plugins[$class] = $plugin;
        $this->logger->info('Enabled plugin', ['plugin' => $class]);

        return $plugin;
    }

    public function shutdown(): void
    {
        $firstFailure = null;
        foreach (array_reverse($this->plugins, true) as $class => $plugin) {
            try {
                $plugin->disable();
                $this->logger->info('Disabled plugin', ['plugin' => $class]);
            } catch (Throwable $error) {
                $firstFailure ??= $error;
                $this->logger->error(
                    'Plugin disable failed',
                    ['plugin' => $class, 'exception' => $error],
                );
            }
        }
        $this->plugins = [];

        if ($firstFailure !== null) {
            throw $firstFailure;
        }
    }
}
