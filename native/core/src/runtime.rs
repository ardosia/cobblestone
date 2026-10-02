use core::num::NonZeroU32;

/// Stable identity of one persistent PHP runtime inside Cobblestone.
///
/// Runtime IDs are internal ownership/routing identities, not plugin-facing concepts.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct RuntimeId(NonZeroU32);

impl RuntimeId {
    /// Creates a runtime ID. Zero is reserved as an invalid/unset value.
    pub const fn new(value: u32) -> Option<Self> {
        match NonZeroU32::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Returns the nonzero integer representation.
    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeId;

    #[test]
    fn zero_is_rejected() {
        assert_eq!(RuntimeId::new(0), None);
    }

    #[test]
    fn nonzero_value_round_trips() {
        let runtime = RuntimeId::new(7).expect("7 is nonzero");
        assert_eq!(runtime.get(), 7);
    }
}
