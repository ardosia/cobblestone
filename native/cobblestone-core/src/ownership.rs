use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;

use crate::{Arena, Handle, InsertError, RuntimeId};

/// Monotonic ownership generation carried by commands that target mutable authority.
///
/// A handle identifies an object. The ownership epoch identifies one specific ownership lifetime
/// of that object. Commands carrying an older epoch are rejected after a transfer.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct OwnershipEpoch(u64);

impl OwnershipEpoch {
    /// Initial epoch assigned to a newly inserted value.
    pub const ZERO: Self = Self(0);

    /// Reconstructs an epoch carried by an internal command or persisted routing record.
    ///
    /// Constructing an epoch does not grant authority. Operations still require the value's
    /// current owner and current epoch to match.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the integer representation.
    pub const fn get(self) -> u64 {
        self.0
    }

    fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

/// Type-safe generational identity for one owner-gated native value.
///
/// Identity alone never grants mutation permission. Callers must also supply the current owner and
/// ownership epoch for authoritative access.
pub struct OwnedHandle<T> {
    inner: Handle<OwnedValue<T>>,
    marker: PhantomData<fn() -> T>,
}

impl<T> OwnedHandle<T> {
    const fn new(inner: Handle<OwnedValue<T>>) -> Self {
        Self {
            inner,
            marker: PhantomData,
        }
    }

    /// Zero-based arena slot index.
    pub const fn index(self) -> u32 {
        self.inner.index()
    }

    /// Nonzero generational identity for the slot.
    pub const fn generation(self) -> u32 {
        self.inner.generation()
    }
}

impl<T> Copy for OwnedHandle<T> {}

impl<T> Clone for OwnedHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> PartialEq for OwnedHandle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<T> Eq for OwnedHandle<T> {}

impl<T> Hash for OwnedHandle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state);
    }
}

impl<T> fmt::Debug for OwnedHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OwnedHandle")
            .field("index", &self.index())
            .field("generation", &self.generation())
            .finish()
    }
}

/// Current routing metadata for one live owner-gated value.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct OwnershipMetadata {
    owner: RuntimeId,
    epoch: OwnershipEpoch,
}

impl OwnershipMetadata {
    /// Runtime currently permitted to mutate or reclaim the value.
    pub const fn owner(self) -> RuntimeId {
        self.owner
    }

    /// Epoch commands must carry for the current ownership lifetime.
    pub const fn epoch(self) -> OwnershipEpoch {
        self.epoch
    }
}

/// Safe rejection modes for owner-gated native operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum OwnershipError {
    /// The generational handle no longer identifies a live value.
    StaleHandle,

    /// The requesting runtime is not the current owner.
    WrongOwner {
        /// Runtime attempting the operation.
        requester: RuntimeId,
        /// Runtime that currently owns the value.
        current: RuntimeId,
    },

    /// The command was issued for an older or otherwise incorrect ownership lifetime.
    StaleEpoch {
        /// Epoch supplied by the command.
        expected: OwnershipEpoch,
        /// Current epoch stored with the authoritative value.
        current: OwnershipEpoch,
    },

    /// The ownership epoch cannot advance without wrapping.
    EpochExhausted,
}

impl fmt::Display for OwnershipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaleHandle => f.write_str("owned handle is stale"),
            Self::WrongOwner { requester, current } => write!(
                f,
                "runtime {} does not own value held by runtime {}",
                requester.get(),
                current.get()
            ),
            Self::StaleEpoch { expected, current } => write!(
                f,
                "ownership epoch mismatch: command={} current={}",
                expected.get(),
                current.get()
            ),
            Self::EpochExhausted => f.write_str("ownership epoch space exhausted"),
        }
    }
}

impl std::error::Error for OwnershipError {}

struct OwnedValue<T> {
    owner: RuntimeId,
    epoch: OwnershipEpoch,
    value: T,
}

/// Generational native storage with exactly one mutable owner per live value.
///
/// The arena supplies mechanism only. It rejects unauthorized access and exposes current ownership
/// metadata so higher semantic layers can choose whether a wrong-owner operation should be routed
/// to the owner or returned as an error.
pub struct OwnedArena<T> {
    arena: Arena<OwnedValue<T>>,
}

impl<T> OwnedArena<T> {
    /// Creates an empty owner-gated arena.
    pub const fn new() -> Self {
        Self {
            arena: Arena::new(),
        }
    }

    /// Number of currently live authoritative values.
    pub const fn len(&self) -> usize {
        self.arena.len()
    }

    /// Whether the arena contains no live authoritative values.
    pub const fn is_empty(&self) -> bool {
        self.arena.is_empty()
    }

    /// Inserts a value with one initial owner and epoch zero.
    pub fn insert(
        &mut self,
        owner: RuntimeId,
        value: T,
    ) -> Result<(OwnedHandle<T>, OwnershipEpoch), InsertError> {
        let handle = self.arena.insert(OwnedValue {
            owner,
            epoch: OwnershipEpoch::ZERO,
            value,
        })?;
        Ok((OwnedHandle::new(handle), OwnershipEpoch::ZERO))
    }

    /// Returns current ownership metadata without granting access to the value.
    ///
    /// Semantic layers may use this information to route a command to the current owner.
    pub fn metadata(&self, handle: OwnedHandle<T>) -> Result<OwnershipMetadata, OwnershipError> {
        let state = self.state(handle)?;
        Ok(OwnershipMetadata {
            owner: state.owner,
            epoch: state.epoch,
        })
    }

    /// Returns shared access only when the requester owns the current ownership epoch.
    pub fn get(
        &self,
        handle: OwnedHandle<T>,
        requester: RuntimeId,
        expected_epoch: OwnershipEpoch,
    ) -> Result<&T, OwnershipError> {
        let state = self.state(handle)?;
        assert_owner_and_epoch(state, requester, expected_epoch)?;
        Ok(&state.value)
    }

    /// Returns mutable access only when the requester owns the current ownership epoch.
    pub fn get_mut(
        &mut self,
        handle: OwnedHandle<T>,
        requester: RuntimeId,
        expected_epoch: OwnershipEpoch,
    ) -> Result<&mut T, OwnershipError> {
        let state = self.state_mut(handle)?;
        assert_owner_and_epoch(state, requester, expected_epoch)?;
        Ok(&mut state.value)
    }

    /// Transfers authority and returns the newly active ownership epoch.
    ///
    /// If the epoch cannot advance, the owner and value remain unchanged.
    pub fn transfer(
        &mut self,
        handle: OwnedHandle<T>,
        requester: RuntimeId,
        expected_epoch: OwnershipEpoch,
        new_owner: RuntimeId,
    ) -> Result<OwnershipEpoch, OwnershipError> {
        let state = self.state_mut(handle)?;
        assert_owner_and_epoch(state, requester, expected_epoch)?;
        let next_epoch = state.epoch.next().ok_or(OwnershipError::EpochExhausted)?;

        state.owner = new_owner;
        state.epoch = next_epoch;
        Ok(next_epoch)
    }

    /// Reclaims a value only for its current owner and ownership epoch.
    pub fn remove(
        &mut self,
        handle: OwnedHandle<T>,
        requester: RuntimeId,
        expected_epoch: OwnershipEpoch,
    ) -> Result<T, OwnershipError> {
        {
            let state = self.state(handle)?;
            assert_owner_and_epoch(state, requester, expected_epoch)?;
        }

        self.arena
            .remove(handle.inner)
            .map(|state| state.value)
            .ok_or(OwnershipError::StaleHandle)
    }

    fn state(&self, handle: OwnedHandle<T>) -> Result<&OwnedValue<T>, OwnershipError> {
        self.arena
            .get(handle.inner)
            .ok_or(OwnershipError::StaleHandle)
    }

    fn state_mut(&mut self, handle: OwnedHandle<T>) -> Result<&mut OwnedValue<T>, OwnershipError> {
        self.arena
            .get_mut(handle.inner)
            .ok_or(OwnershipError::StaleHandle)
    }

    #[cfg(test)]
    fn force_epoch_for_test(
        &mut self,
        handle: OwnedHandle<T>,
        epoch: OwnershipEpoch,
    ) -> Result<(), OwnershipError> {
        self.state_mut(handle)?.epoch = epoch;
        Ok(())
    }
}

impl<T> Default for OwnedArena<T> {
    fn default() -> Self {
        Self::new()
    }
}

fn assert_owner_and_epoch<T>(
    state: &OwnedValue<T>,
    requester: RuntimeId,
    expected_epoch: OwnershipEpoch,
) -> Result<(), OwnershipError> {
    if state.owner != requester {
        return Err(OwnershipError::WrongOwner {
            requester,
            current: state.owner,
        });
    }
    if state.epoch != expected_epoch {
        return Err(OwnershipError::StaleEpoch {
            expected: expected_epoch,
            current: state.epoch,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{NativeBuffer, RuntimeId};

    use super::{OwnedArena, OwnershipEpoch, OwnershipError};

    fn runtime(value: u32) -> RuntimeId {
        RuntimeId::new(value).expect("test runtime is nonzero")
    }

    #[test]
    fn transfer_rejects_wrong_owner_and_stale_epoch() {
        let one = runtime(1);
        let two = runtime(2);
        let mut arena = OwnedArena::new();
        let (handle, epoch) = arena.insert(one, 10_u64).expect("slot available");

        assert!(matches!(
            arena.get_mut(handle, two, epoch),
            Err(OwnershipError::WrongOwner { .. })
        ));

        *arena.get_mut(handle, one, epoch).expect("owner can mutate") = 11;
        let next = arena
            .transfer(handle, one, epoch, two)
            .expect("owner can transfer");

        assert_eq!(next.get(), 1);
        assert!(matches!(
            arena.get_mut(handle, two, epoch),
            Err(OwnershipError::StaleEpoch { .. })
        ));
        assert!(matches!(
            arena.get_mut(handle, one, next),
            Err(OwnershipError::WrongOwner { .. })
        ));
        assert_eq!(
            *arena
                .get(handle, two, next)
                .expect("new owner can read current value"),
            11
        );
    }

    #[test]
    fn removal_rejects_stale_handle_after_slot_reuse() {
        let one = runtime(1);
        let mut arena = OwnedArena::new();
        let (stale, epoch) = arena.insert(one, 1_u64).expect("slot available");
        assert_eq!(arena.remove(stale, one, epoch), Ok(1));

        let (replacement, replacement_epoch) = arena.insert(one, 2_u64).expect("slot reusable");
        assert_eq!(stale.index(), replacement.index());
        assert_ne!(stale.generation(), replacement.generation());
        assert_eq!(arena.metadata(stale), Err(OwnershipError::StaleHandle));
        assert_eq!(
            *arena
                .get(replacement, one, replacement_epoch)
                .expect("replacement is live"),
            2
        );
    }

    #[test]
    fn wrong_owner_or_epoch_cannot_reclaim_live_value() {
        let one = runtime(1);
        let two = runtime(2);
        let mut arena = OwnedArena::new();
        let (handle, epoch) = arena.insert(one, 7_u64).expect("slot available");

        assert!(matches!(
            arena.remove(handle, two, epoch),
            Err(OwnershipError::WrongOwner { .. })
        ));
        assert!(matches!(
            arena.remove(handle, one, OwnershipEpoch::new(9)),
            Err(OwnershipError::StaleEpoch { .. })
        ));
        assert_eq!(
            *arena
                .get(handle, one, epoch)
                .expect("failed reclaim must leave value live"),
            7
        );
    }

    #[test]
    fn epoch_exhaustion_leaves_owner_unchanged() {
        let one = runtime(1);
        let two = runtime(2);
        let mut arena = OwnedArena::new();
        let (handle, _) = arena.insert(one, 5_u64).expect("slot available");
        let max = OwnershipEpoch::new(u64::MAX);
        arena
            .force_epoch_for_test(handle, max)
            .expect("live test handle");

        assert_eq!(
            arena.transfer(handle, one, max, two),
            Err(OwnershipError::EpochExhausted)
        );
        assert_eq!(
            arena.metadata(handle).expect("value remains live").owner(),
            one
        );
        assert_eq!(
            arena.metadata(handle).expect("value remains live").epoch(),
            max
        );
    }

    #[test]
    fn immutable_clone_survives_transfer_without_granting_old_owner_access() {
        let one = runtime(1);
        let two = runtime(2);
        let mut arena = OwnedArena::new();
        let buffer = NativeBuffer::from_vec(vec![1, 2, 3]);
        let (handle, epoch) = arena.insert(one, buffer).expect("slot available");

        let shared = arena
            .get(handle, one, epoch)
            .expect("owner can read")
            .clone();
        let next = arena
            .transfer(handle, one, epoch, two)
            .expect("owner can transfer");

        assert!(matches!(
            arena.get(handle, one, next),
            Err(OwnershipError::WrongOwner { .. })
        ));
        let authoritative = arena.get(handle, two, next).expect("new owner can read");
        assert!(authoritative.shares_storage_with(&shared));
        assert_eq!(shared.as_slice(), &[1, 2, 3]);
    }
}
