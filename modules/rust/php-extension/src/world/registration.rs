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
        .function(wrap_function!(cobblestone_world_sky_light))
        .function(wrap_function!(cobblestone_world_set_sky_light))
        .function(wrap_function!(cobblestone_world_fill_sky_light_from))
        .function(wrap_function!(cobblestone_world_block_light))
        .function(wrap_function!(cobblestone_world_set_block_light))
        .function(wrap_function!(cobblestone_world_height_map))
        .function(wrap_function!(cobblestone_world_recalculate_height_map))
        .function(wrap_function!(cobblestone_world_block_extra_data))
        .function(wrap_function!(cobblestone_world_set_block_extra_data));

    let module = super::block::register(module);
    let module = super::patch::register(module);
    let module = super::residency::register(module);
    let module = super::revision::register(module);
    super::snapshot::register(module)
}
