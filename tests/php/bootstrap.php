<?php

declare(strict_types=1);

$autoload = dirname(__DIR__, 2) . '/vendor/autoload.php';
if (!is_file($autoload)) {
    fwrite(STDERR, "Composer autoloader is missing; run composer dump-autoload from the repository root.\n");
    exit(1);
}

require_once $autoload;
