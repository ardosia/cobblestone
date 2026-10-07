use std::panic::{AssertUnwindSafe, catch_unwind};

use ext_php_rs::exception::{PhpException, PhpResult};

pub(crate) fn php_error(message: impl Into<String>) -> PhpException {
    PhpException::default(message.into())
}

fn catch_native_panic<T>(operation: impl FnOnce() -> PhpResult<T>) -> Result<PhpResult<T>, ()> {
    catch_unwind(AssertUnwindSafe(operation)).map_err(|_| ())
}

/// Contains every Cobblestone-owned panic before control returns to the generated Zend handler.
pub(crate) fn php_boundary<T>(operation: impl FnOnce() -> PhpResult<T>) -> PhpResult<T> {
    match catch_native_panic(operation) {
        Ok(result) => result,
        Err(()) => Err(php_error("Cobblestone native panic contained")),
    }
}

#[cfg(test)]
mod tests {
    use super::catch_native_panic;

    #[test]
    fn catches_panics_before_the_zend_boundary() {
        let result = catch_native_panic::<()>(|| panic!("boundary test panic"));
        assert!(result.is_err());
    }

    #[test]
    fn preserves_successful_results() {
        let result = catch_native_panic(|| Ok(42_u32)).expect("operation did not panic");
        assert_eq!(result.expect("operation succeeded"), 42);
    }
}
