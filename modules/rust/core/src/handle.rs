use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::num::NonZeroU32;

/// A type-safe identity into a generational arena.
///
/// Equality and hashing use only the slot index and generation. The type parameter prevents a
/// handle for one native identity class from being passed to another arena accidentally.
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

    /// Zero-based arena slot index.
    pub const fn index(self) -> u32 {
        self.index
    }

    /// Nonzero generation associated with the slot when this handle was issued.
    pub const fn generation(self) -> u32 {
        self.generation.get()
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
