use std::collections::HashMap;

use crate::{OwnershipEpoch, RuntimeId};

/// Stable identity of one gameplay execution region.
///
/// Mapping chunks to this identity is a gameplay-layer concern. Native core only tracks the
/// resulting identity and its current owning PHP runtime.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct RegionId {
    x: i32,
    z: i32,
}

impl RegionId {
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    pub const fn x(self) -> i32 {
        self.x
    }

    pub const fn z(self) -> i32 {
        self.z
    }
}

/// Current native routing record for one assigned execution region.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct RegionRoute {
    owner: RuntimeId,
    epoch: OwnershipEpoch,
}

impl RegionRoute {
    pub const fn owner(self) -> RuntimeId {
        self.owner
    }

    pub const fn epoch(self) -> OwnershipEpoch {
        self.epoch
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RegionRouteError {
    AlreadyAssigned,
    Unassigned,
    WrongOwner {
        requester: RuntimeId,
        current: RuntimeId,
    },
    StaleEpoch {
        expected: OwnershipEpoch,
        current: OwnershipEpoch,
    },
    EpochExhausted,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct RegionRouteState {
    owner: Option<RuntimeId>,
    epoch: OwnershipEpoch,
}

/// Scheduler-owned directory mapping semantic region identities to PHP runtime owners.
///
/// This is routing mechanism only. It contains no chunks, blocks, entities, or gameplay callbacks.
/// Unassignment advances the epoch and leaves a tombstone so reassigning the same coordinates cannot
/// make an older route valid again.
#[derive(Debug, Default)]
pub struct RegionDirectory {
    routes: HashMap<RegionId, RegionRouteState>,
}

impl RegionDirectory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.routes
            .values()
            .filter(|route| route.owner.is_some())
            .count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn assign(
        &mut self,
        region: RegionId,
        owner: RuntimeId,
    ) -> Result<OwnershipEpoch, RegionRouteError> {
        match self.routes.get_mut(&region) {
            Some(route) if route.owner.is_some() => Err(RegionRouteError::AlreadyAssigned),
            Some(route) => {
                route.owner = Some(owner);
                Ok(route.epoch)
            }
            None => {
                self.routes.insert(
                    region,
                    RegionRouteState {
                        owner: Some(owner),
                        epoch: OwnershipEpoch::ZERO,
                    },
                );
                Ok(OwnershipEpoch::ZERO)
            }
        }
    }

    pub fn route(&self, region: RegionId) -> Option<RegionRoute> {
        let state = self.routes.get(&region)?;
        Some(RegionRoute {
            owner: state.owner?,
            epoch: state.epoch,
        })
    }

    pub fn transfer(
        &mut self,
        region: RegionId,
        requester: RuntimeId,
        expected_epoch: OwnershipEpoch,
        new_owner: RuntimeId,
    ) -> Result<OwnershipEpoch, RegionRouteError> {
        let route = self
            .routes
            .get_mut(&region)
            .ok_or(RegionRouteError::Unassigned)?;
        let current_owner = route.owner.ok_or(RegionRouteError::Unassigned)?;

        if current_owner != requester {
            return Err(RegionRouteError::WrongOwner {
                requester,
                current: current_owner,
            });
        }
        if route.epoch != expected_epoch {
            return Err(RegionRouteError::StaleEpoch {
                expected: expected_epoch,
                current: route.epoch,
            });
        }

        let next = next_epoch(route.epoch)?;
        route.owner = Some(new_owner);
        route.epoch = next;
        Ok(next)
    }

    pub fn unassign(
        &mut self,
        region: RegionId,
        requester: RuntimeId,
        expected_epoch: OwnershipEpoch,
    ) -> Result<OwnershipEpoch, RegionRouteError> {
        let route = self
            .routes
            .get_mut(&region)
            .ok_or(RegionRouteError::Unassigned)?;
        let current_owner = route.owner.ok_or(RegionRouteError::Unassigned)?;

        if current_owner != requester {
            return Err(RegionRouteError::WrongOwner {
                requester,
                current: current_owner,
            });
        }
        if route.epoch != expected_epoch {
            return Err(RegionRouteError::StaleEpoch {
                expected: expected_epoch,
                current: route.epoch,
            });
        }

        let next = next_epoch(route.epoch)?;
        route.owner = None;
        route.epoch = next;
        Ok(next)
    }
}

fn next_epoch(epoch: OwnershipEpoch) -> Result<OwnershipEpoch, RegionRouteError> {
    epoch
        .get()
        .checked_add(1)
        .map(OwnershipEpoch::new)
        .ok_or(RegionRouteError::EpochExhausted)
}

#[cfg(test)]
mod tests {
    use super::{RegionDirectory, RegionId, RegionRouteError};
    use crate::{OwnershipEpoch, RuntimeId};

    fn runtime(value: u32) -> RuntimeId {
        RuntimeId::new(value).expect("test runtime is nonzero")
    }

    #[test]
    fn region_routes_have_exactly_one_owner_and_epoch() {
        let one = runtime(1);
        let region = RegionId::new(-2, 7);
        let mut directory = RegionDirectory::new();

        let epoch = directory.assign(region, one).expect("region assign");
        assert_eq!(epoch, OwnershipEpoch::ZERO);

        let route = directory.route(region).expect("route exists");
        assert_eq!(route.owner(), one);
        assert_eq!(route.epoch(), OwnershipEpoch::ZERO);
        assert_eq!(directory.len(), 1);
    }

    #[test]
    fn transfer_advances_epoch_and_rejects_stale_or_wrong_owner() {
        let one = runtime(1);
        let two = runtime(2);
        let region = RegionId::new(3, -4);
        let mut directory = RegionDirectory::new();

        let epoch = directory.assign(region, one).unwrap();
        assert!(matches!(
            directory.transfer(region, two, epoch, two),
            Err(RegionRouteError::WrongOwner { .. })
        ));

        let next = directory.transfer(region, one, epoch, two).unwrap();
        assert_eq!(next.get(), 1);
        assert!(matches!(
            directory.transfer(region, two, epoch, one),
            Err(RegionRouteError::StaleEpoch { .. })
        ));

        let route = directory.route(region).unwrap();
        assert_eq!(route.owner(), two);
        assert_eq!(route.epoch(), next);
    }

    #[test]
    fn unassign_and_reassign_never_revalidate_stale_epoch() {
        let one = runtime(1);
        let two = runtime(2);
        let region = RegionId::new(0, 0);
        let mut directory = RegionDirectory::new();

        let first = directory.assign(region, one).unwrap();
        let tombstone = directory.unassign(region, one, first).unwrap();
        assert_eq!(tombstone.get(), first.get() + 1);
        assert_eq!(directory.route(region), None);
        assert!(directory.is_empty());

        let replacement = directory.assign(region, two).unwrap();
        assert_eq!(replacement, tombstone);
        assert!(matches!(
            directory.transfer(region, two, first, one),
            Err(RegionRouteError::StaleEpoch { .. })
        ));
        assert_eq!(directory.route(region).unwrap().owner(), two);
    }
}
