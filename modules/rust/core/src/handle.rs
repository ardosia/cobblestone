use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::num::NonZeroU32;

/// A type-safe identity into a generational arena.
///
/// Equality and hashing use only the slot index and generation. The type parameter prevents a
/// handle for one native identity class from being passed to another arena accidentally.
pub struct Handle<T> {
    index: u32,
    generation: NonZeroU32,
    marker: PhantomData<fn() -> T>,
}

impl<T> Handle<T> {
    const fn new(index: u32, generation: NonZeroU32) -> Self {
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

/// Failure to allocate a new arena slot.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum InsertError {
    /// The arena already contains the maximum number of addressable slots.
    CapacityExhausted,
}

struct Slot<T> {
    generation: NonZeroU32,
    value: Option<T>,
    retired: bool,
}

/// Private generational storage used to back stable native identities.
///
/// Removing a value invalidates all existing handles to that slot before the slot becomes
/// reusable. When the generation reaches `u32::MAX`, the slot is retired rather than wrapped.
pub struct Arena<T> {
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
    len: usize,
}

impl<T> Arena<T> {
    /// Creates an empty arena.
    pub const fn new() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            len: 0,
        }
    }

    /// Number of currently live values.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the arena contains no live values.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Inserts a value and returns its stable generational handle.
    pub fn insert(&mut self, value: T) -> Result<Handle<T>, InsertError> {
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            debug_assert!(!slot.retired);
            debug_assert!(slot.value.is_none());
            slot.value = Some(value);
            self.len += 1;
            return Ok(Handle::new(index, slot.generation));
        }

        let index = u32::try_from(self.slots.len()).map_err(|_| InsertError::CapacityExhausted)?;
        let generation = NonZeroU32::MIN;
        self.slots.push(Slot {
            generation,
            value: Some(value),
            retired: false,
        });
        self.len += 1;
        Ok(Handle::new(index, generation))
    }

    /// Returns a shared reference if the handle is live and its generation matches.
    pub fn get(&self, handle: Handle<T>) -> Option<&T> {
        let slot = self.slots.get(handle.index as usize)?;
        if slot.generation != handle.generation {
            return None;
        }
        slot.value.as_ref()
    }

    /// Returns a mutable reference if the handle is live and its generation matches.
    pub fn get_mut(&mut self, handle: Handle<T>) -> Option<&mut T> {
        let slot = self.slots.get_mut(handle.index as usize)?;
        if slot.generation != handle.generation {
            return None;
        }
        slot.value.as_mut()
    }

    /// Whether the handle currently identifies a live value.
    pub fn contains(&self, handle: Handle<T>) -> bool {
        self.get(handle).is_some()
    }

    /// Removes a live value, invalidating the handle before the slot can be reused.
    pub fn remove(&mut self, handle: Handle<T>) -> Option<T> {
        let slot = self.slots.get_mut(handle.index as usize)?;
        if slot.generation != handle.generation {
            return None;
        }

        let value = slot.value.take()?;
        self.len -= 1;

        if let Some(next) = slot
            .generation
            .get()
            .checked_add(1)
            .and_then(NonZeroU32::new)
        {
            slot.generation = next;
            self.free.push(handle.index);
        } else {
            slot.retired = true;
        }

        Some(value)
    }
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU32;

    use super::{Arena, Handle};

    #[derive(Debug, Eq, PartialEq)]
    struct Entity(u32);

    #[test]
    fn stale_handle_is_rejected_after_slot_reuse() {
        let mut arena = Arena::new();
        let first = arena.insert(Entity(1)).expect("slot available");

        assert_eq!(arena.remove(first), Some(Entity(1)));
        assert!(!arena.contains(first));

        let second = arena.insert(Entity(2)).expect("slot reusable");
        assert_eq!(first.index(), second.index());
        assert_ne!(first.generation(), second.generation());
        assert_eq!(arena.get(first), None);
        assert_eq!(arena.get(second), Some(&Entity(2)));
    }

    #[test]
    fn wrong_generation_cannot_remove_live_value() {
        let mut arena = Arena::new();
        let live = arena.insert(Entity(9)).expect("slot available");
        let wrong_generation = NonZeroU32::new(live.generation() + 1).expect("still nonzero");
        let stale = Handle::new(live.index(), wrong_generation);

        assert_eq!(arena.remove(stale), None);
        assert_eq!(arena.get(live), Some(&Entity(9)));
    }

    #[test]
    fn generation_exhaustion_retires_slot() {
        let mut arena = Arena::new();
        let issued = arena.insert(Entity(3)).expect("slot available");
        let slot = &mut arena.slots[issued.index() as usize];
        slot.generation = NonZeroU32::MAX;
        let max_generation = Handle::new(issued.index(), NonZeroU32::MAX);

        assert_eq!(arena.remove(max_generation), Some(Entity(3)));
        assert!(arena.slots[issued.index() as usize].retired);
        assert!(arena.free.is_empty());

        let next = arena.insert(Entity(4)).expect("new slot available");
        assert_ne!(next.index(), issued.index());
    }
}
