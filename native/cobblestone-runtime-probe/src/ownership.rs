use core::fmt;

use cobblestone_core::{Arena, Handle, InsertError, RuntimeId};

struct ProbeState {
    owner: RuntimeId,
    epoch: u64,
    value: u64,
}

/// Opaque identity for one C003 ownership probe.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct ProbeHandle(Handle<ProbeState>);

impl ProbeHandle {
    pub const fn index(self) -> u32 {
        self.0.index()
    }

    pub const fn generation(self) -> u32 {
        self.0.generation()
    }
}

/// Observable state used by the C003 ownership torture harness.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct ProbeSnapshot {
    owner: RuntimeId,
    epoch: u64,
    value: u64,
}

impl ProbeSnapshot {
    pub const fn owner(self) -> RuntimeId {
        self.owner
    }

    pub const fn epoch(self) -> u64 {
        self.epoch
    }

    pub const fn value(self) -> u64 {
        self.value
    }
}

/// Safe rejection modes for owner-gated C003 probe operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum OwnershipError {
    StaleHandle,
    WrongOwner {
        requester: RuntimeId,
        current: RuntimeId,
    },
    StaleEpoch {
        expected: u64,
        current: u64,
    },
    EpochExhausted,
}

impl fmt::Display for OwnershipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaleHandle => f.write_str("ownership probe handle is stale"),
            Self::WrongOwner { requester, current } => write!(
                f,
                "runtime {} does not own probe held by runtime {}",
                requester.get(),
                current.get()
            ),
            Self::StaleEpoch { expected, current } => write!(
                f,
                "ownership epoch mismatch: command={expected} current={current}"
            ),
            Self::EpochExhausted => f.write_str("ownership epoch space exhausted"),
        }
    }
}

impl std::error::Error for OwnershipError {}

/// C003-private owner-gated native probe storage.
///
/// This is deliberately narrower than the eventual C004 ownership model. It exists to torture
/// stale-handle and ownership-transfer behavior before production gameplay depends on it.
pub struct OwnedProbeArena {
    arena: Arena<ProbeState>,
}

impl OwnedProbeArena {
    pub const fn new() -> Self {
        Self {
            arena: Arena::new(),
        }
    }

    pub const fn len(&self) -> usize {
        self.arena.len()
    }

    /// Returns true when no ownership probes are live.
    pub const fn is_empty(&self) -> bool {
        self.arena.is_empty()
    }

    pub fn create(&mut self, owner: RuntimeId, value: u64) -> Result<ProbeHandle, InsertError> {
        self.arena
            .insert(ProbeState {
                owner,
                epoch: 0,
                value,
            })
            .map(ProbeHandle)
    }

    pub fn snapshot(&self, handle: ProbeHandle) -> Result<ProbeSnapshot, OwnershipError> {
        let state = self
            .arena
            .get(handle.0)
            .ok_or(OwnershipError::StaleHandle)?;
        Ok(ProbeSnapshot {
            owner: state.owner,
            epoch: state.epoch,
            value: state.value,
        })
    }

    pub fn mutate(
        &mut self,
        handle: ProbeHandle,
        requester: RuntimeId,
        expected_epoch: u64,
        value: u64,
    ) -> Result<(), OwnershipError> {
        let state = self
            .arena
            .get_mut(handle.0)
            .ok_or(OwnershipError::StaleHandle)?;
        assert_owner_and_epoch(state, requester, expected_epoch)?;
        state.value = value;
        Ok(())
    }

    pub fn transfer(
        &mut self,
        handle: ProbeHandle,
        requester: RuntimeId,
        expected_epoch: u64,
        new_owner: RuntimeId,
    ) -> Result<u64, OwnershipError> {
        let state = self
            .arena
            .get_mut(handle.0)
            .ok_or(OwnershipError::StaleHandle)?;
        assert_owner_and_epoch(state, requester, expected_epoch)?;

        let next_epoch = state
            .epoch
            .checked_add(1)
            .ok_or(OwnershipError::EpochExhausted)?;
        state.owner = new_owner;
        state.epoch = next_epoch;
        Ok(next_epoch)
    }

    pub fn remove(
        &mut self,
        handle: ProbeHandle,
        requester: RuntimeId,
        expected_epoch: u64,
    ) -> Result<ProbeSnapshot, OwnershipError> {
        {
            let state = self
                .arena
                .get(handle.0)
                .ok_or(OwnershipError::StaleHandle)?;
            assert_owner_and_epoch(state, requester, expected_epoch)?;
        }

        let state = self
            .arena
            .remove(handle.0)
            .ok_or(OwnershipError::StaleHandle)?;
        Ok(ProbeSnapshot {
            owner: state.owner,
            epoch: state.epoch,
            value: state.value,
        })
    }
}

impl Default for OwnedProbeArena {
    fn default() -> Self {
        Self::new()
    }
}

fn assert_owner_and_epoch(
    state: &ProbeState,
    requester: RuntimeId,
    expected_epoch: u64,
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
    use cobblestone_core::RuntimeId;

    use super::{OwnedProbeArena, OwnershipError};

    fn runtime(value: u32) -> RuntimeId {
        RuntimeId::new(value).expect("test runtime is nonzero")
    }

    #[test]
    fn ownership_transfer_rejects_wrong_owner_and_stale_epoch() {
        let one = runtime(1);
        let two = runtime(2);
        let mut arena = OwnedProbeArena::new();
        let handle = arena.create(one, 10).expect("probe slot available");

        assert!(matches!(
            arena.mutate(handle, two, 0, 11),
            Err(OwnershipError::WrongOwner { .. })
        ));
        let epoch = arena
            .transfer(handle, one, 0, two)
            .expect("owner can transfer probe");
        assert_eq!(epoch, 1);
        assert!(matches!(
            arena.mutate(handle, two, 0, 12),
            Err(OwnershipError::StaleEpoch { .. })
        ));
        arena
            .mutate(handle, two, epoch, 13)
            .expect("new owner with current epoch can mutate");
        assert_eq!(arena.snapshot(handle).expect("live probe").value(), 13);
    }

    #[test]
    fn removed_handle_stays_stale_after_slot_reuse() {
        let one = runtime(1);
        let mut arena = OwnedProbeArena::new();
        let stale = arena.create(one, 1).expect("probe slot available");
        arena.remove(stale, one, 0).expect("owner can remove probe");

        let replacement = arena.create(one, 2).expect("slot reusable");
        assert_eq!(stale.index(), replacement.index());
        assert_ne!(stale.generation(), replacement.generation());
        assert_eq!(arena.snapshot(stale), Err(OwnershipError::StaleHandle));
        assert_eq!(arena.snapshot(replacement).expect("live probe").value(), 2);
    }
}
