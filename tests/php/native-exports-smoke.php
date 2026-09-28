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
    'cobblestone_session_protocol84_send_native_chunks',
    'cobblestone_session_protocol84_player_spawned',
    'cobblestone_session_protocol84_track_move_player',
    'cobblestone_session_protocol84_send_prepared_view_chunks',
    'cobblestone_session_protocol84_flush_world_changes',
    'cobblestone_world_create',
    'cobblestone_world_storage_attach',
    'cobblestone_world_storage_prepare_loads',
    'cobblestone_world_storage_request_load',
    'cobblestone_world_storage_tick',
    'cobblestone_world_storage_stats',
    'cobblestone_world_storage_flush',
    'cobblestone_world_destroy',
    'cobblestone_world_ensure_chunk',
    'cobblestone_world_lifecycle_flags',
    'cobblestone_world_set_lifecycle_flags',
    'cobblestone_world_pin_chunk',
    'cobblestone_world_unpin_chunk',
    'cobblestone_world_chunk_pin_count',
    'cobblestone_world_chunk_dirty',
    'cobblestone_world_mark_persisted',
    'cobblestone_world_try_evict_chunk',
    'cobblestone_world_terrain_revision',
    'cobblestone_world_light_revision',
    'cobblestone_world_commit_terrain_revision',
    'cobblestone_world_commit_light_revision',
    'cobblestone_world_block_state',
    'cobblestone_world_set_block_state',
    'cobblestone_world_fill_layers',
    'cobblestone_world_biome',
    'cobblestone_world_set_biome',
    'cobblestone_world_fill_biome',
    'cobblestone_world_sky_light',
    'cobblestone_world_set_sky_light',
    'cobblestone_world_fill_sky_light_from',
    'cobblestone_world_block_light',
    'cobblestone_world_set_block_light',
    'cobblestone_world_height_map',
    'cobblestone_world_recalculate_height_map',
    'cobblestone_world_block_extra_data',
    'cobblestone_world_set_block_extra_data',
    'cobblestone_world_apply_patch',
    'cobblestone_world_snapshot',
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
