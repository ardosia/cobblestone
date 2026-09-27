<?php

declare(strict_types=1);

function export_fail(string $message): never
{
    fwrite(STDERR, $message . PHP_EOL);
    exit(1);
}

if (!extension_loaded('cobblestone_core_php')) {
    export_fail('cobblestone_core_php extension did not load for export smoke test');
}

$required = [
    'cobblestone_core_runtime_id',
    'cobblestone_core_async_submit',
    'cobblestone_core_async_ready',
    'cobblestone_core_async_take',
    'cobblestone_session_start',
    'cobblestone_session_running',
    'cobblestone_session_poll_event',
    'cobblestone_session_send',
    'cobblestone_session_protocol84_accept_login',
    'cobblestone_session_protocol84_spawn_probe',
    'cobblestone_session_protocol84_accept_login_world',
    'cobblestone_session_protocol84_request_chunk_radius',
    'cobblestone_session_protocol84_send_initial_chunks',
    'cobblestone_session_disconnect',
    'cobblestone_session_stop',
];

foreach ($required as $function) {
    if (!function_exists($function)) {
        export_fail("missing required native export: {$function}");
    }
}

$forbidden = [
    'cobblestone_session_protocol_84_accept_login',
    'cobblestone_session_protocol_84_spawn_probe',
];

foreach ($forbidden as $function) {
    if (function_exists($function)) {
        export_fail("unexpected renamed native export: {$function}");
    }
}

printf(
    "cobblestone-core-php: exports=%d protocol84_names=stable composer_namespace=independent\n",
    count($required),
);
