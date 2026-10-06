<?php

declare(strict_types=1);

namespace Cobblestone\Application;

use Cobblestone\Config\ServerConfig;
use Cobblestone\Config\StorageConfig;
use Cobblestone\Config\WorldConfig;
use Cobblestone\World\Generator\WorldSeed;
use InvalidArgumentException;

final readonly class ApplicationConfig
{
    public function __construct(
        public ServerConfig $server,
        public WorldConfig $world,
        public StorageConfig $storage,
        public string $logLevel = 'INFO',
    ) {
        if (trim($logLevel) === '') {
            throw new InvalidArgumentException('log level must not be empty');
        }
    }

    /** @param array<string, string> $environment */
    public static function fromEnvironment(string $projectRoot, array $environment): self
    {
        $projectRoot = rtrim($projectRoot, DIRECTORY_SEPARATOR);
        if ($projectRoot === '') {
            throw new InvalidArgumentException('project root must not be empty');
        }

        $seedInput = self::text($environment, 'COBBLESTONE_WORLD_SEED', '');

        return new self(
            new ServerConfig(
                bind: self::text($environment, 'COBBLESTONE_BIND', '0.0.0.0:19132'),
                maxConnections: self::integer($environment, 'COBBLESTONE_MAX_CONNECTIONS', 20, 1, PHP_INT_MAX),
                name: self::text($environment, 'COBBLESTONE_SERVER_NAME', 'Cobblestone'),
                initialChunkRadius: self::integer($environment, 'COBBLESTONE_VIEW_DISTANCE', 3, 1, 3),
                tickRate: self::integer($environment, 'COBBLESTONE_TICK_RATE', 20, 1, 1_000),
            ),
            new WorldConfig(
                directory: self::text(
                    $environment,
                    'COBBLESTONE_WORLD_DIR',
                    $projectRoot . DIRECTORY_SEPARATOR . 'worlds' . DIRECTORY_SEPARATOR . 'world',
                ),
                name: self::text($environment, 'COBBLESTONE_WORLD_NAME', 'Cobblestone'),
                seed: WorldSeed::fromTargetInput($seedInput, WorldSeed::random()),
            ),
            new StorageConfig(
                saveWorkers: self::integer($environment, 'COBBLESTONE_SAVE_WORKERS', 2, 1, 32),
                loadWorkers: self::integer($environment, 'COBBLESTONE_LOAD_WORKERS', 2, 1, 32),
                compactionMinDeadBytes: self::integer(
                    $environment,
                    'COBBLESTONE_COMPACTION_MIN_DEAD_BYTES',
                    StorageConfig::DEFAULT_COMPACTION_MIN_DEAD_BYTES,
                    0,
                    PHP_INT_MAX,
                ),
                compactionMinDeadPercent: self::integer(
                    $environment,
                    'COBBLESTONE_COMPACTION_MIN_DEAD_PERCENT',
                    StorageConfig::DEFAULT_COMPACTION_MIN_DEAD_PERCENT,
                    0,
                    100,
                ),
            ),
            self::text($environment, 'COBBLESTONE_LOG_LEVEL', 'INFO'),
        );
    }

    public static function fromProcessEnvironment(string $projectRoot): self
    {
        $environment = getenv();
        if (!is_array($environment)) {
            $environment = [];
        }

        /** @var array<string, string> $environment */
        return self::fromEnvironment($projectRoot, $environment);
    }

    /** @param array<string, string> $environment */
    private static function text(array $environment, string $key, string $default): string
    {
        $value = $environment[$key] ?? '';

        return $value === '' ? $default : $value;
    }

    /** @param array<string, string> $environment */
    private static function integer(
        array $environment,
        string $key,
        int $default,
        int $minimum,
        int $maximum,
    ): int {
        $raw = $environment[$key] ?? '';
        if ($raw === '') {
            return $default;
        }
        if (!preg_match('/^\d+$/', $raw)) {
            throw new InvalidArgumentException("{$key} must be an integer in range {$minimum}..{$maximum}");
        }

        $value = filter_var(
            $raw,
            FILTER_VALIDATE_INT,
            ['options' => ['min_range' => $minimum, 'max_range' => $maximum]],
        );
        if ($value === false) {
            throw new InvalidArgumentException("{$key} must be an integer in range {$minimum}..{$maximum}");
        }

        return $value;
    }
}
