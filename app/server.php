<?php

declare(strict_types=1);

use Cobblestone\Application\Application;
use Cobblestone\Application\ApplicationConfig;

$root = dirname(__DIR__);
$autoload = $root . '/vendor/autoload.php';
if (!is_file($autoload)) {
    fwrite(STDERR, "Composer autoloader is missing; run composer setup.\n");
    exit(1);
}

require_once $autoload;

try {
    Application::create(ApplicationConfig::fromProcessEnvironment($root))->run();
} catch (Throwable $error) {
    fwrite(STDERR, 'Cobblestone startup failed: ' . $error->getMessage() . PHP_EOL);
    exit(1);
}
