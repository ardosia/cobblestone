pub(crate) mod bridge;
mod gameplay;
mod join;
mod legacy;
mod view;

use ext_php_rs::prelude::*;

pub(crate) fn register(module: ModuleBuilder) -> ModuleBuilder {
    legacy::register(view::register(join::register(gameplay::register(
        bridge::register(module),
    ))))
}

pub(crate) fn shutdown() {
    bridge::shutdown();
}
