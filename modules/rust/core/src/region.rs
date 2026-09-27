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

/// Current native routing record for one execution region.
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

/// Scheduler-owned directory mapping semantic region identities to PHP runtime owners.
///
/// This is routing mechanism only. It contains no chunks, blocks, entities, or gameplay callbacks.
#[derive(Debug, Default)]
pub struct RegionDirectory {
    routes: HashMap<RegionId, RegionRoute>,
}

impl RegionDirectory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.routes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    pub fn assign(
        &mut self,
        region: RegionId,
        owner: RuntimeId,
    ) -> Result<OwnershipEpoch, RegionRouteError> {
        if self.routes.contains_key(&region) {
            return Err(RegionRouteError::AlreadyAssigned);
        }

        self.routes.insert(
            region,
            RegionRoute {
                owner,
                epoch: OwnershipEpoch::ZERO,
            },
        );
        Ok(OwnershipEpoch::ZERO)
    }

    pub fn route(&self, region: RegionId) -> Option<RegionRoute> {
        self.routes.get(&region).copied()
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

        if route.owner != requester {
            return Err(RegionRouteError::WrongOwner {
                requester,
                current: route.owner,
            });
        }
        if route.epoch != expected_epoch {
            return Err(RegionRouteError::StaleEpoch {
                expected: expected_epoch,
                current: route.epoch,
            });
        }

        let next = route
            .epoch
            .get()
            .checked_add(1)
            .map(OwnershipEpoch::new)
            .ok_or(RegionRouteError::EpochExhausted)?;

        route.owner = new_owner;
        route.epoch = next;
        Ok(next)
    }

    pub fn unassign(
        &mut self,
        region: RegionId,
        requester: RuntimeId,
        expected_epoch: OwnershipEpoch,
    ) -> Result<(), RegionRouteError> {
        let route = self
            .routes
            .get(&region)
            .copied()
            .ok_or(RegionRouteError::Unassigned)?;

        if route.owner != requester {
            return Err(RegionRouteError::WrongOwner {
                requester,
                current: route.owner,
            });
        }
        if route.epoch != expected_epoch {
            return Err(RegionRouteError::StaleEpoch {
                expected: expected_epoch,
                current: route.epoch,
            });
        }

        self.routes.remove(&region);
        Ok(())
    }
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
    fn unassign_requires_current_owner_and_epoch() {
        let one = runtime(1);
        let region = RegionId::new(0, 0);
        let mut directory = RegionDirectory::new();
        let epoch = directory.assign(region, one).unwrap();

        assert_eq!(directory.unassign(region, one, epoch), Ok(()));
        assert_eq!(directory.route(region), None);
        assert!(directory.is_empty());
    }
}
