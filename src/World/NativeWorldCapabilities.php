<?php

declare(strict_types=1);

namespace Cobblestone\World;

/**
 * Native world-store extension capability probe.
 *
 * @internal
 */
final class NativeWorldCapabilities
{
    /** @var list<string> */
    private const REQUIRED_FUNCTIONS = [
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
    ];

    private function __construct()
    {
    }

    public static function available(): bool
    {
        if (!extension_loaded('cobblestone_core_php')) {
            return false;
        }

        foreach (self::REQUIRED_FUNCTIONS as $function) {
            if (!\function_exists($function)) {
                return false;
            }
        }

        return true;
    }
}
