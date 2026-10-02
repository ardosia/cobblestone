use std::num::NonZeroU64;

/// Stable identity assigned to one accepted gameplay session.
///
/// Session IDs are process-local and never intentionally reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(pub(crate) NonZeroU64);

impl SessionId {
    /// Creates a session identity from a previously issued nonzero value.
    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Returns the nonzero integer representation.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}
