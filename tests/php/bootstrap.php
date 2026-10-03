<?php

declare(strict_types=1);

$autoload = dirname(__DIR__, 2) . '/vendor/autoload.php';
if (!is_file($autoload)) {
    fwrite(STDERR, "Composer autoloader is missing; run composer dump-autoload from the repository root.\n");
    exit(1);
}

require_once $autoload;

/** @internal Test-only access to the server orchestration boundary. */
function testServerRuntime(Cobblestone\Server\Server $server): Cobblestone\Server\Internal\Runtime
{
    $property = new ReflectionProperty(Cobblestone\Server\Server::class, 'runtime');
    $runtime = $property->getValue($server);
    if (!$runtime instanceof Cobblestone\Server\Internal\Runtime) {
        throw new RuntimeException('server runtime is unavailable');
    }

    return $runtime;
}

/** @internal Test-only access for white-box session gameplay integration tests. */
function testServerGameplay(Cobblestone\Server\Server $server): Cobblestone\Session\Internal\Gameplay
{
    $runtime = testServerRuntime($server);
    $property = new ReflectionProperty(Cobblestone\Server\Internal\Runtime::class, 'gameplay');
    $gameplay = $property->getValue($runtime);
    if (!$gameplay instanceof Cobblestone\Session\Internal\Gameplay) {
        throw new RuntimeException('server gameplay runtime is unavailable');
    }

    return $gameplay;
}

/** @internal Test-only access for deterministic native disconnect integration tests. */
function testNativeSessions(Cobblestone\Server\Server $server): Cobblestone\Native\Session
{
    $runtime = testServerRuntime($server);
    $property = new ReflectionProperty(Cobblestone\Server\Internal\Runtime::class, 'sessions');
    $sessions = $property->getValue($runtime);
    if (!$sessions instanceof Cobblestone\Native\Session) {
        throw new RuntimeException('native session runtime is unavailable');
    }

    return $sessions;
}
