<?php

declare(strict_types=1);

require __DIR__ . '/bootstrap.php';

use Cobblestone\Log\ContextLogger;
use Cobblestone\Log\SpringBootFormatter;
use Monolog\Handler\StreamHandler;
use Monolog\Level;
use Monolog\Logger;

$stream = fopen('php://memory', 'w+');
if ($stream === false) {
    throw new RuntimeException('failed to create memory stream');
}

$handler = new StreamHandler($stream, Level::Debug);
$handler->setFormatter(new SpringBootFormatter());
$root = new Logger('Cobblestone', [$handler]);
$logger = new ContextLogger(
    $root,
    'Cobblestone.Test.Logging',
    ['component' => 'smoke'],
);

$logger->info('hello {subject}', ['subject' => 'world', 'answer' => 42]);
rewind($stream);
$output = stream_get_contents($stream);

if (
    $output === false
    || !str_contains($output, ' INFO ')
    || !str_contains($output, '--- [Cobblestone] [')
    || !str_contains($output, 'Cobblestone.Test.Logging')
    || !str_contains($output, ': hello world')
    || !str_contains($output, 'answer=42')
    || !str_contains($output, 'component="smoke"')
) {
    throw new RuntimeException('Spring Boot-style structured log output mismatch');
}

fwrite(STDOUT, "log-smoke: passed\n");