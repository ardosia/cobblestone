#![cfg_attr(windows, feature(abi_vectorcall))]

mod boundary;
mod diagnostics;
mod runtime;
mod session;
mod world;

use std::panic::{AssertUnwindSafe, catch_unwind};

use ext_php_rs::prelude::*;

unsafe extern "C" fn cobblestone_core_shutdown(_type: i32, _module_number: i32) -> i32 {
    match catch_unwind(AssertUnwindSafe(|| {
        diagnostics::shutdown();
        session::shutdown();
        world::shutdown();
    })) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

#[php_module]
pub fn get_module(module: ModuleBuilder) -> ModuleBuilder {
    let module = module
        .name("cobblestone_core_php")
        .version(env!("CARGO_PKG_VERSION"))
        .shutdown_function(cobblestone_core_shutdown);

    world::register(session::register(diagnostics::register(module)))
}
