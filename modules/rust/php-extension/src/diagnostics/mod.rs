mod async_tasks;
mod probes;

use cobblestone_core::NativeBuffer;
use ext_php_rs::exception::PhpResult;
use ext_php_rs::prelude::*;

use crate::boundary::{php_boundary, php_error};
use crate::runtime::current_runtime_id;

#[php_function]
pub fn cobblestone_core_ping() -> PhpResult<i64> {
    php_boundary(|| Ok(0))
}

#[php_function]
pub fn cobblestone_core_buffer_copy_len(value: String) -> PhpResult<i64> {
    php_boundary(|| {
        let buffer = NativeBuffer::copy_from_slice(value.as_bytes());
        i64::try_from(buffer.len())
            .map_err(|_| php_error("Cobblestone native buffer length exceeds PHP integer range"))
    })
}

#[php_function]
pub fn cobblestone_core_runtime_id() -> PhpResult<u32> {
    php_boundary(|| {
        current_runtime_id()
            .map(cobblestone_core::RuntimeId::get)
            .map_err(php_error)
    })
}

pub(crate) fn register(module: ModuleBuilder) -> ModuleBuilder {
    let module = module
        .function(wrap_function!(cobblestone_core_ping))
        .function(wrap_function!(cobblestone_core_buffer_copy_len))
        .function(wrap_function!(cobblestone_core_runtime_id));
    async_tasks::register(probes::register(module))
}

pub(crate) fn shutdown() {
    async_tasks::shutdown();
}
