pub(crate) mod bridge;
mod sync;
mod work;

use ext_php_rs::prelude::*;

pub(crate) fn register(module: ModuleBuilder) -> ModuleBuilder {
    sync::register(work::register(bridge::register(module)))
}

pub(crate) fn shutdown() {
    bridge::shutdown();
}
