<?php

declare(strict_types=1);

$finder = PhpCsFixer\Finder::create()
    ->files()
    ->name('*.php')
    ->in([
        __DIR__ . '/src',
        __DIR__ . '/tests/php',
        __DIR__ . '/tools',
    ])
    ->append([new SplFileInfo(__DIR__ . '/bin/cobblestone')]);

return (new PhpCsFixer\Config())
    ->setRiskyAllowed(false)
    ->setRules([
        '@PER-CS2.0' => true,
    ])
    ->setFinder($finder);
