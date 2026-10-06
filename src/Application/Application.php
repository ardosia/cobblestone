<?php

declare(strict_types=1);

namespace Cobblestone\Application;

use Cobblestone\Internal\Target;
use Cobblestone\Log\LoggerFactory;
use Cobblestone\Server\Server;
use Cobblestone\Server\WorldFactory;
use Psr\Log\LoggerInterface;

final class Application
{
    private readonly LoggerInterface $logger;

    private function __construct(
        private readonly ApplicationConfig $config,
        private readonly Server $server,
        LoggerFactory $logs,
    ) {
        $this->logger = $logs->logger('Cobblestone.Application');
    }

    public static function create(ApplicationConfig $config): self
    {
        $logs = LoggerFactory::console($config->logLevel);
        $world = WorldFactory::persistentInfinite(
            $config->world->directory,
            $config->world->name,
            $config->world->seed,
            $config->storage,
        );
        $server = Server::create($config->server, world: $world, logs: $logs);

        return new self($config, $server, $logs);
    }

    public function run(): int
    {
        $this->logger->info(
            'Configured Cobblestone',
            [
                'bind' => $this->config->server->bind,
                'protocol' => Target::GAME_PROTOCOL,
                'raknet' => Target::RAKNET_PROTOCOL,
                'tick_rate' => $this->config->server->tickRate,
                'world' => $this->server->world()->name(),
                'world_seed' => $this->server->world()->seed(),
                'world_generator' => $this->server->world()->generator()->name(),
                'world_dir' => $this->config->world->directory,
                'view_distance_chunks' => $this->config->server->initialChunkRadius,
                'compaction_min_dead_bytes' => $this->config->storage->compactionMinDeadBytes,
                'compaction_min_dead_percent' => $this->config->storage->compactionMinDeadPercent,
            ],
        );

        return $this->server->run();
    }
}
