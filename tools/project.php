<?php

declare(strict_types=1);

const ROOT = __DIR__ . '/..';
const NATIVE_EXTENSION = ROOT . '/native/extension';

function fail(string $message, int $code = 1): never
{
    fwrite(STDERR, "cobblestone: {$message}" . PHP_EOL);
    exit($code);
}

/** @param list<string> $command */
function run(array $command, ?string $cwd = null): void
{
    $process = proc_open(
        $command,
        [
            0 => STDIN,
            1 => STDOUT,
            2 => STDERR,
        ],
        $pipes,
        $cwd ?? ROOT,
    );

    if (!is_resource($process)) {
        fail('failed to start command: ' . implode(' ', $command));
    }

    $code = proc_close($process);
    if ($code !== 0) {
        fail('command failed with exit code ' . $code . ': ' . implode(' ', $command), $code);
    }
}

function nativeToolchain(): string
{
    return PHP_OS_FAMILY === 'Windows' ? 'nightly-2026-09-20' : '1.98.0';
}

function extensionPath(): string
{
    return match (PHP_OS_FAMILY) {
        'Windows' => NATIVE_EXTENSION . '/target/release/cobblestone_core_php.dll',
        'Darwin' => NATIVE_EXTENSION . '/target/release/libcobblestone_core_php.dylib',
        default => NATIVE_EXTENSION . '/target/release/libcobblestone_core_php.so',
    };
}

function setupComposer(): void
{
    run(['composer', 'update', '--no-interaction']);
}

function ensureAutoload(): void
{
    if (!is_file(ROOT . '/vendor/autoload.php')) {
        fail('Composer packages are not installed; run composer setup');
    }

    run(['composer', 'dump-autoload', '--no-interaction', '--classmap-authoritative']);
}

function validateComposer(): void
{
    run(['composer', 'validate', '--strict', '--no-check-publish', ROOT . '/composer.json']);
}

function buildNative(): void
{
    run(['cargo', '+' . nativeToolchain(), 'build', '--release'], NATIVE_EXTENSION);
}

function checkNative(): void
{
    $toolchain = '+' . nativeToolchain();

    run(['cargo', $toolchain, 'fmt', '--check'], NATIVE_EXTENSION);
    run(['cargo', $toolchain, 'check'], NATIVE_EXTENSION);
    run(['cargo', $toolchain, 'clippy', '--', '-D', 'warnings'], NATIVE_EXTENSION);
}

function lintPhp(): void
{
    $roots = [
        ROOT . '/src',
        ROOT . '/tests/php',
        ROOT . '/tools',
    ];
    $files = [ROOT . '/bin/cobblestone'];

    foreach ($roots as $root) {
        if (!is_dir($root)) {
            continue;
        }

        $iterator = new RecursiveIteratorIterator(
            new RecursiveDirectoryIterator($root, FilesystemIterator::SKIP_DOTS),
        );

        foreach ($iterator as $file) {
            if ($file->isFile() && strtolower($file->getExtension()) === 'php') {
                $files[] = $file->getPathname();
            }
        }
    }

    sort($files);

    foreach (array_unique($files) as $file) {
        run([PHP_BINARY, '-l', $file]);
    }
}

/** @param list<string> $arguments */
function runWithExtension(string $script, array $arguments = []): void
{
    $extension = extensionPath();
    if (!is_file($extension)) {
        fail('native extension is missing; run composer native:build');
    }

    run([
        PHP_BINARY,
        '-n',
        '-d',
        'extension=' . $extension,
        $script,
        ...$arguments,
    ]);
}

function testPhp(): void
{
    run([PHP_BINARY, ROOT . '/tests/php/zts-probe.php']);

    foreach ([
        'biome-smoke.php',
        'command-smoke.php',
        'log-smoke.php',
        'plugin-smoke.php',
        'scheduler-smoke.php',
        'tick-smoke.php',
        'world-composition-smoke.php',
        'world-block-catalog-smoke.php',
        'world-light-smoke.php',
        'world-parity-smoke.php',
        'world-seed-smoke.php',
        'world-smoke.php',
        'world-mutation-smoke.php',
    ] as $test) {
        run([PHP_BINARY, ROOT . '/tests/php/unit/' . $test]);
    }

    foreach ([
        'native-exports-smoke.php',
        'extension-smoke.php',
        'native-world-smoke.php',
        'native-biome-source-smoke.php',
        'native-infinite-smoke.php',
        'native-storage-smoke.php',
        'fiber-smoke.php',
        'session-runtime-smoke.php',
    ] as $test) {
        runWithExtension(ROOT . '/tests/php/native/' . $test);
    }

    foreach ([
        'world-sync-smoke.php',
        'multi-view-smoke.php',
        'pending-disconnect-smoke.php',
        'backpressure-view-smoke.php',
        'persistent-join-smoke.php',
        'server-smoke.php',
    ] as $test) {
        runWithExtension(ROOT . '/tests/php/integration/' . $test);
        if ($test === 'world-sync-smoke.php') {
            runWithExtension(ROOT . '/tests/php/integration/' . $test, ['--transition-only']);
            runWithExtension(ROOT . '/tests/php/integration/' . $test, ['--radius-cycle']);
            runWithExtension(ROOT . '/tests/php/integration/' . $test, ['--stream-torture']);
        }
    }
}

function listModules(): void
{
    printf("php: src (ardosia/cobblestone)%s", PHP_EOL);

    $rustRoot = ROOT . '/native';
    $rust = [];
    if (is_dir($rustRoot)) {
        foreach (new DirectoryIterator($rustRoot) as $entry) {
            if (!$entry->isDot() && $entry->isDir()) {
                $rust[] = $entry->getFilename();
            }
        }
    }
    sort($rust);

    foreach ($rust as $module) {
        printf("rust: %s%s", $module, PHP_EOL);
    }
}

$command = $argv[1] ?? null;

switch ($command) {
    case 'setup':
        setupComposer();
        break;

    case 'native:build':
        buildNative();
        break;

    case 'native:check':
        checkNative();
        break;

    case 'build':
        ensureAutoload();
        buildNative();
        break;

    case 'check':
        validateComposer();
        lintPhp();
        run(['python', 'tools/ci.py', 'all']);
        checkNative();
        break;

    case 'test:php':
        ensureAutoload();
        testPhp();
        break;

    case 'test':
        ensureAutoload();
        buildNative();
        testPhp();
        break;

    case 'verify':
        run([PHP_BINARY, __FILE__, 'check']);
        run([PHP_BINARY, __FILE__, 'test']);
        break;

    case 'serve':
        ensureAutoload();
        buildNative();
        runWithExtension(ROOT . '/bin/cobblestone', array_slice($argv, 2));
        break;

    case 'modules':
        listModules();
        break;

    default:
        fail(
            'usage: composer {setup|build|check|modules|native:build|native:check|serve|test|test:php|verify}',
            64,
        );
}