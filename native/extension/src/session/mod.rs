pub(crate) mod bridge;
mod gameplay;
mod join;
mod view;

use ext_php_rs::prelude::*;

pub(crate) fn register(module: ModuleBuilder) -> ModuleBuilder {
    view::register(join::register(gameplay::register(bridge::register(module))))
}

pub(crate) fn shutdown() {
    bridge::shutdown();
}
