use std::panic::{AssertUnwindSafe, catch_unwind};

use ext_php_rs::exception::{PhpException, PhpResult};

pub(crate) fn php_error(message: impl Into<String>) -> PhpException {
    PhpException::default(message.into())
}

/// Contains every Cobblestone-owned panic before control returns to the generated Zend handler.
pub(crate) fn php_boundary<T>(operation: impl FnOnce() -> PhpResult<T>) -> PhpResult<T> {
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(result) => result,
        Err(_) => Err(php_error("Cobblestone native panic contained")),
    }
}
