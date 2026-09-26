use std::error::Error;
use std::io;
use std::time::Instant;

use cobblestone_core::{OwnedArena, OwnedHandle, OwnershipError, OwnershipEpoch, RuntimeId};

const ITERATIONS: u64 = 250_000;

fn main() -> Result<(), Box<dyn Error>> {
    let runtime_one = RuntimeId::new(1).ok_or_else(|| io::Error::other("runtime id 1 invalid"))?;
    let runtime_two = RuntimeId::new(2).ok_or_else(|| io::Error::other("runtime id 2 invalid"))?;
    let mut arena = OwnedArena::new();
    let mut retained_stale = Vec::with_capacity(usize::try_from(ITERATIONS)?);
    let mut wrong_owner_rejections = 0_u64;
    let mut stale_epoch_rejections = 0_u64;
    let started = Instant::now();

    for sequence in 0..ITERATIONS {
        let (handle, initial_epoch) = arena
            .insert(runtime_one, sequence)
            .map_err(|error| io::Error::other(format!("owned insert failed: {error:?}")))?;
        if initial_epoch != OwnershipEpoch::ZERO {
            return Err(io::Error::other("new ownership epoch was not zero").into());
        }
        let initial = arena.metadata(handle)?;
        if initial.owner() != runtime_one
            || initial.epoch() != OwnershipEpoch::ZERO
            || *arena.get(handle, runtime_one, initial_epoch)? != sequence
        {
            return Err(io::Error::other("new owned arena state was corrupted").into());
        }

        match arena.get_mut(handle, runtime_two, initial_epoch) {
            Err(OwnershipError::WrongOwner { .. }) => wrong_owner_rejections += 1,
            other => {
                return Err(io::Error::other(format!(
                    "wrong-owner mutation was not rejected: {other:?}"
                ))
                .into());
            }
        }

        *arena.get_mut(handle, runtime_one, initial_epoch)? = sequence + 2;
        let new_epoch = arena.transfer(handle, runtime_one, initial_epoch, runtime_two)?;
        if new_epoch.get() != 1 {
            return Err(io::Error::other(format!(
                "unexpected transfer epoch {} at sequence {sequence}",
                new_epoch.get()
            ))
            .into());
        }

        match arena.get_mut(handle, runtime_two, initial_epoch) {
            Err(OwnershipError::StaleEpoch { .. }) => stale_epoch_rejections += 1,
            other => {
                return Err(io::Error::other(format!(
                    "stale ownership epoch was not rejected: {other:?}"
                ))
                .into());
            }
        }
        match arena.get_mut(handle, runtime_one, new_epoch) {
            Err(OwnershipError::WrongOwner { .. }) => wrong_owner_rejections += 1,
            other => {
                return Err(io::Error::other(format!(
                    "previous owner mutation was not rejected: {other:?}"
                ))
                .into());
            }
        }

        *arena.get_mut(handle, runtime_two, new_epoch)? = sequence + 5;
        let removed = arena.remove(handle, runtime_two, new_epoch)?;
        if removed != sequence + 5 {
            return Err(io::Error::other("removed owned value was corrupted").into());
        }
        assert_stale(&arena, handle)?;
        retained_stale.push(handle);

        let (replacement, replacement_epoch) = arena
            .insert(runtime_one, sequence)
            .map_err(|error| io::Error::other(format!("replacement insert failed: {error:?}")))?;
        if replacement.index() != handle.index() || replacement.generation() == handle.generation()
        {
            return Err(io::Error::other("generational slot reuse invariant failed").into());
        }
        arena.remove(replacement, runtime_one, replacement_epoch)?;
    }

    if !arena.is_empty() {
        return Err(io::Error::other("owned arena leaked live values").into());
    }
    for handle in retained_stale {
        assert_stale(&arena, handle)?;
    }

    println!(
        "ownership-torture: iterations={ITERATIONS} wrong_owner_rejections={wrong_owner_rejections} stale_epoch_rejections={stale_epoch_rejections} elapsed_ms={}",
        started.elapsed().as_millis()
    );
    Ok(())
}

fn assert_stale(
    arena: &OwnedArena<u64>,
    handle: OwnedHandle<u64>,
) -> Result<(), Box<dyn Error>> {
    match arena.metadata(handle) {
        Err(OwnershipError::StaleHandle) => Ok(()),
        other => {
            Err(io::Error::other(format!("stale handle unexpectedly resolved: {other:?}")).into())
        }
    }
}
