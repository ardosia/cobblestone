use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::num::NonZeroU32;

/// Stable generational identity for one native arena slot.
///
/// Handles carry no mutable authority. The phantom type prevents a handle for one native identity
/// class from being passed to another arena accidentally.
pub struct Handle<T> {
    pub(crate) index: u32,
    pub(crate) generation: NonZeroU32,
    marker: PhantomData<fn() -> T>,
}

impl<T> Handle<T> {
    pub(crate) const fn new(index: u32, generation: NonZeroU32) -> Self {
        Self {
            index,
            generation,
            marker: PhantomData,
        }
    }

    pub const fn index(self) -> u32 {
        self.index
    }

    /// Nonzero generation associated with the slot when this handle was issued.
    pub const fn generation(self) -> u32 {
        self.generation.get()
    }

    /// Packs this opaque identity into a process-local scalar suitable for FFI transport.
    pub const fn into_raw(self) -> u64 {
        (self.generation.get() as u64) << 32 | self.index as u64
    }

    /// Reconstructs a type-safe handle from an opaque process-local scalar.
    pub const fn from_raw(raw: u64) -> Option<Self> {
        let generation = (raw >> 32) as u32;
        match NonZeroU32::new(generation) {
            Some(generation) => Some(Self::new(raw as u32, generation)),
            None => None,
        }
    }
}

impl<T> Copy for Handle<T> {}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}

impl<T> Eq for Handle<T> {}

impl<T> Hash for Handle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
    }
}

impl<T> fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Handle")
            .field("index", &self.index)
            .field("generation", &self.generation)
            .finish()
    }
}
