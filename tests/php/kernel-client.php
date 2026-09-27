<?php

declare(strict_types=1);

use Cobblestone\Kernel\ServerKernel;
use Cobblestone\Kernel\SessionConnected;
use Cobblestone\Kernel\SessionDisconnected;
use Cobblestone\Kernel\SessionLoginAccepted;
use Cobblestone\Kernel\SessionSpawned;

require_once __DIR__ . '/bootstrap.php';

if (!extension_loaded('cobblestone_core_php')) {
    fwrite(STDERR, "cobblestone_core_php extension is not loaded\n");
    exit(1);
}

$bind = getenv('COBBLESTONE_BIND') ?: '0.0.0.0:19132';
$maxConnections = (int) (getenv('COBBLESTONE_MAX_CONNECTIONS') ?: '20');
$kernel = ServerKernel::start($bind, $maxConnections, 'Cobblestone');

$kernel->events()->listen(
    SessionConnected::class,
    static function (object $event): void {
        if ($event instanceof SessionConnected) {
            printf("cobblestone-kernel-client: connected session=%d peer=%s\n", $event->sessionId, $event->peer);
        }
    },
);
$kernel->events()->listen(
    SessionLoginAccepted::class,
    static function (object $event): void {
        if ($event instanceof SessionLoginAccepted) {
            printf("cobblestone-kernel-client: login-accepted session=%d protocol=84\n", $event->sessionId);
        }
    },
);
$kernel->events()->listen(
    SessionSpawned::class,
    static function (object $event): void {
        if ($event instanceof SessionSpawned) {
            printf(
                "cobblestone-kernel-client: spawned session=%d requested_radius=%d probe_radius=%d\n",
                $event->sessionId,
                $event->requestedRadius,
                $event->probeRadius,
            );
        }
    },
);
$kernel->events()->listen(
    SessionDisconnected::class,
    static function (object $event): void {
        if ($event instanceof SessionDisconnected) {
            printf(
                "cobblestone-kernel-client: disconnected session=%d reason=%s\n",
                $event->sessionId,
                $event->reason,
            );
        }
    },
);

printf("cobblestone-kernel-client: listening=%s protocol=84 raknet=8\n", $bind);
printf("cobblestone-kernel-client: waiting for fixed 0.15.10 client\n");

while (true) {
    $started = hrtime(true);
    $kernel->tick(256);
    $remaining = 50_000_000 - (hrtime(true) - $started);
    if ($remaining > 0) {
        usleep((int) ($remaining / 1_000));
    }
}
