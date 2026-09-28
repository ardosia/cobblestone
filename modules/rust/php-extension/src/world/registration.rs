use ext_php_rs::prelude::*;

use super::*;

pub(super) fn register(module: ModuleBuilder) -> ModuleBuilder {
    let module = module
        .function(wrap_function!(cobblestone_world_create))
        .function(wrap_function!(cobblestone_world_destroy));

    let module = super::block::register(module);
    let module = super::block_extra::register(module);
    let module = super::height::register(module);
    let module = super::light::register(module);
    let module = super::patch::register(module);
    let module = super::residency::register(module);
    let module = super::revision::register(module);
    let module = super::snapshot::register(module);
    super::storage::register(module)
}
