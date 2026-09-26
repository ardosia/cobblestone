use cobblestone_core::{
    InsertError, OwnedArena, OwnedHandle, OwnershipEpoch, RuntimeId,
};

pub use cobblestone_core::OwnershipError;

/// Opaque identity for one C003 ownership probe.
pub type ProbeHandle = OwnedHandle<u64>;

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

/// C003 compatibility wrapper over the production C004 owner-gated arena.
///
/// Keeping the torture harness on this wrapper preserves its experiment-facing API while making
/// the stress workload exercise the production cobblestone-core ownership implementation.
pub struct OwnedProbeArena {
    arena: OwnedArena<u64>,
}

impl OwnedProbeArena {
    pub const fn new() -> Self {
        Self {
            arena: OwnedArena::new(),
        }
    }

    pub const fn len(&self) -> usize {
        self.arena.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.arena.is_empty()
    }

    pub fn create(&mut self, owner: RuntimeId, value: u64) -> Result<ProbeHandle, InsertError> {
        self.arena.insert(owner, value).map(|(handle, _)| handle)
    }

    pub fn snapshot(&self, handle: ProbeHandle) -> Result<ProbeSnapshot, OwnershipError> {
        let metadata = self.arena.metadata(handle)?;
        let value = *self
            .arena
            .get(handle, metadata.owner(), metadata.epoch())?;
        Ok(ProbeSnapshot {
            owner: metadata.owner(),
            epoch: metadata.epoch().get(),
            value,
        })
    }

    pub fn mutate(
        &mut self,
        handle: ProbeHandle,
        requester: RuntimeId,
        expected_epoch: u64,
        value: u64,
    ) -> Result<(), OwnershipError> {
        *self.arena.get_mut(
            handle,
            requester,
            OwnershipEpoch::new(expected_epoch),
        )? = value;
        Ok(())
    }

    pub fn transfer(
        &mut self,
        handle: ProbeHandle,
        requester: RuntimeId,
        expected_epoch: u64,
        new_owner: RuntimeId,
    ) -> Result<u64, OwnershipError> {
        self.arena
            .transfer(
                handle,
                requester,
                OwnershipEpoch::new(expected_epoch),
                new_owner,
            )
            .map(OwnershipEpoch::get)
    }

    pub fn remove(
        &mut self,
        handle: ProbeHandle,
        requester: RuntimeId,
        expected_epoch: u64,
    ) -> Result<ProbeSnapshot, OwnershipError> {
        let value = self
            .arena
            .remove(handle, requester, OwnershipEpoch::new(expected_epoch))?;
        Ok(ProbeSnapshot {
            owner: requester,
            epoch: expected_epoch,
            value,
        })
    }
}

impl Default for OwnedProbeArena {
    fn default() -> Self {
        Self::new()
    }
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
