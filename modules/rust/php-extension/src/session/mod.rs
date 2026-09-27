pub(crate) mod bridge;
mod join;
mod legacy;

use ext_php_rs::prelude::*;

pub(crate) fn register(module: ModuleBuilder) -> ModuleBuilder {
    legacy::register(join::register(bridge::register(module)))
}

pub(crate) fn shutdown() {
    bridge::shutdown();
}
