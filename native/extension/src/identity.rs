use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::{php_boundary, php_error};
use crate::runtime::current_runtime_id;

#[php_function]
pub fn cobblestone_core_runtime_id() -> PhpResult<u32> {
    php_boundary(|| {
        current_runtime_id()
            .map(cobblestone_runtime::RuntimeId::get)
            .map_err(php_error)
    })
}

pub(crate) fn register(module: ModuleBuilder) -> ModuleBuilder {
    module.function(wrap_function!(cobblestone_core_runtime_id))
}
