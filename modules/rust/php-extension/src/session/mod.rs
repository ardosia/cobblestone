pub(crate) mod bridge;
mod join;

use ext_php_rs::prelude::*;

pub(crate) fn register(module: ModuleBuilder) -> ModuleBuilder {
    join::register(bridge::register(module))
}

pub(crate) fn shutdown() {
    bridge::shutdown();
}
