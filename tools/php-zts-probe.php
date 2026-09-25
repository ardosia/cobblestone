<?php

declare(strict_types=1);

if (PHP_MAJOR_VERSION !== 8 || PHP_MINOR_VERSION !== 5) {
    fwrite(STDERR, sprintf("expected PHP 8.5.x, got %s\n", PHP_VERSION));
    exit(1);
}

if (PHP_ZTS !== true || ZEND_THREAD_SAFE !== true) {
    fwrite(STDERR, "expected a thread-safe/ZTS PHP build\n");
    exit(1);
}

printf(
    "php-zts: version=%s os=%s zts=%s debug=%s\n",
    PHP_VERSION,
    PHP_OS_FAMILY,
    PHP_ZTS ? "yes" : "no",
    PHP_DEBUG ? "yes" : "no",
);
