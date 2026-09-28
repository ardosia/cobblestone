use ext_php_rs::prelude::*;

use super::*;

pub(super) fn register(module: ModuleBuilder) -> ModuleBuilder {
    let module = module
        .function(wrap_function!(cobblestone_world_create))
        .function(wrap_function!(cobblestone_world_storage_attach))
        .function(wrap_function!(cobblestone_world_storage_prepare_loads))
        .function(wrap_function!(cobblestone_world_storage_request_load))
        .function(wrap_function!(cobblestone_world_storage_tick))
        .function(wrap_function!(cobblestone_world_storage_stats))
        .function(wrap_function!(cobblestone_world_storage_flush))
        .function(wrap_function!(cobblestone_world_destroy))
        .function(wrap_function!(cobblestone_world_ensure_chunk))
        .function(wrap_function!(cobblestone_world_lifecycle_flags))
        .function(wrap_function!(cobblestone_world_set_lifecycle_flags))
        .function(wrap_function!(cobblestone_world_pin_chunk))
        .function(wrap_function!(cobblestone_world_unpin_chunk))
        .function(wrap_function!(cobblestone_world_chunk_pin_count))
        .function(wrap_function!(cobblestone_world_chunk_dirty))
        .function(wrap_function!(cobblestone_world_mark_persisted))
        .function(wrap_function!(cobblestone_world_try_evict_chunk))
        .function(wrap_function!(cobblestone_world_terrain_revision))
        .function(wrap_function!(cobblestone_world_light_revision))
        .function(wrap_function!(cobblestone_world_commit_terrain_revision))
        .function(wrap_function!(cobblestone_world_commit_light_revision))
        .function(wrap_function!(cobblestone_world_block_state))
        .function(wrap_function!(cobblestone_world_set_block_state))
        .function(wrap_function!(cobblestone_world_fill_layers))
        .function(wrap_function!(cobblestone_world_biome))
        .function(wrap_function!(cobblestone_world_set_biome))
        .function(wrap_function!(cobblestone_world_fill_biome))
        .function(wrap_function!(cobblestone_world_sky_light))
        .function(wrap_function!(cobblestone_world_set_sky_light))
        .function(wrap_function!(cobblestone_world_fill_sky_light_from))
        .function(wrap_function!(cobblestone_world_block_light))
        .function(wrap_function!(cobblestone_world_set_block_light))
        .function(wrap_function!(cobblestone_world_height_map))
        .function(wrap_function!(cobblestone_world_recalculate_height_map))
        .function(wrap_function!(cobblestone_world_block_extra_data))
        .function(wrap_function!(cobblestone_world_set_block_extra_data));

    let module = super::patch::register(module);
    super::snapshot::register(module)
}
